//! The document pipeline: an open document in, an analysis out.
//!
//! Language detection, URI handling, and position mapping live here so that
//! every analysis resolves a document the same way.

use std::path::{Path, PathBuf};

use lspf::PositionEncoding;
use lspf::types::{Position, Range, Uri};
use lspf_analysis_core::health::{FileHealth, HealthConfig, file_health};
use lspf_analysis_core::{LANG, get_from_ext, get_function_spaces};

/// Maps an LSP language id to a supported language.
///
/// Editors are the authority on what a buffer is, so the language id wins;
/// the extension is only consulted when the id is one this server does not
/// know, which is the common case for clients that report `plaintext`.
pub fn language_for(language_id: &str, path: &Path) -> Option<LANG> {
    let from_id = match language_id {
        "cpp" => Some(LANG::Cpp),
        "rust" => Some(LANG::Rust),
        "python" => Some(LANG::Python),
        "java" => Some(LANG::Java),
        "javascript" | "javascriptreact" => Some(LANG::Javascript),
        "typescript" => Some(LANG::Typescript),
        "typescriptreact" => Some(LANG::Tsx),
        _ => None,
    };
    from_id.or_else(|| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .and_then(get_from_ext)
    })
}

/// Parses `text` and scores it. Returns `None` when the source cannot be
/// parsed into spaces at all.
///
/// This is the CPU-bound half of the server: parsing is synchronous, so
/// callers run it off the protocol task.
pub fn analyze(
    language: LANG,
    text: String,
    path: &Path,
    config: &HealthConfig,
) -> Option<FileHealth> {
    let started = std::time::Instant::now();
    tracing::debug!(?language, path = %path.display(), bytes = text.len(), "analyzing document");
    let space = get_function_spaces(&language, text.into_bytes(), path)?;
    let report = file_health(&space, path, config);
    tracing::debug!(path = %path.display(), functions = report.functions.len(), quality = report.quality,
        elapsed_ms = started.elapsed().as_millis(), "document analysis complete");
    Some(report)
}

/// Recovers a filesystem-like path from a document URI.
///
/// The path is used for language detection and for reporting, not for
/// reading the file — the text always comes from the editor — so a URI that
/// does not name a real file still yields something usable.
pub fn uri_to_path(uri: &Uri) -> PathBuf {
    let raw = uri.as_str();
    // Skip the scheme and authority: `file:///a/b.rs` has to become `/a/b.rs`
    // and not `///a/b.rs`.
    let after_scheme = raw.find("://").map_or_else(
        || raw.find(':').map_or(0, |index| index + 1),
        |index| index + 3,
    );
    let rest = &raw[after_scheme..];
    let path = match rest.find('/') {
        Some(index) if after_scheme > 0 && raw[..after_scheme].ends_with("://") => &rest[index..],
        _ => rest,
    };
    // Trim a query or fragment; neither is part of the path.
    let path = path
        .split_once(['?', '#'])
        .map_or(path, |(before, _)| before);
    PathBuf::from(percent_decode(path))
}

/// Decodes `%XX` escapes. Invalid escapes are left as written rather than
/// dropped, so a path is never silently corrupted.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(byte) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Returns the range covering the content of a 1-based line of `text`.
///
/// A `FuncSpace` records lines, not columns, so a function's diagnostic is
/// drawn across its signature line: that is where a reader looks to find out
/// which function is being complained about.
pub fn line_range(text: &str, line: usize, encoding: PositionEncoding) -> Range {
    let content = line_content(text, line).unwrap_or("");
    let line = u32::try_from(line.saturating_sub(1)).unwrap_or(u32::MAX);
    Range::new(
        Position::new(line, 0),
        Position::new(line, width(content, encoding)),
    )
}

/// Returns the range covering `name` where it is declared on a 1-based line.
///
/// A `FuncSpace` records lines, not columns, so the name's own columns have
/// to be recovered from the signature. The first whole-word occurrence is
/// the declaration: whatever precedes it on the line is a keyword, a
/// modifier, or an assignment target for the same thing.
///
/// Returns `None` when the name is not on that line at all, which is the
/// case for an anonymous function and for a name the parser qualified.
pub fn name_range(
    text: &str,
    line: usize,
    name: &str,
    encoding: PositionEncoding,
) -> Option<Range> {
    if name.is_empty() {
        return None;
    }
    let content = line_content(text, line)?;
    let start = whole_word(content, name)?;
    let line = u32::try_from(line.saturating_sub(1)).unwrap_or(u32::MAX);
    let before = width(&content[..start], encoding);
    Some(Range::new(
        Position::new(line, before),
        Position::new(line, before.saturating_add(width(name, encoding))),
    ))
}

/// Finds the 1-based line on which a space declares its name, given the
/// lines it spans.
///
/// A declaration does not always begin on the line its space does. Java
/// folds annotations and modifiers into the declaration node, so
/// `@Override` is a method's first line and the name is on a later one; the
/// same holds for an annotated class. The name is looked for on each line of
/// the header until it is found.
///
/// Lines that are nothing but an annotation are skipped, so a name that also
/// appears inside one — `@SuppressWarnings("plain")` over `int plain()` —
/// does not stand in for the declaration itself. A name spelled inside a
/// multi-line annotation still can, which is the limit of reading this off
/// the text rather than the tree.
///
/// Returns `None` when the name is on none of those lines, which is the case
/// for an anonymous function and for a name the parser qualified.
pub fn declaration_line(
    text: &str,
    start_line: usize,
    end_line: usize,
    name: &str,
) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    (start_line..=end_line.max(start_line)).find(|&line| {
        line_content(text, line).is_some_and(|content| {
            !content.trim_start().starts_with('@') && whole_word(content, name).is_some()
        })
    })
}

/// Finds where a space declares its name, and the range covering it.
///
/// The line comes from [`declaration_line`]; the columns from
/// [`name_range`].
pub fn declaration_range(
    text: &str,
    start_line: usize,
    end_line: usize,
    name: &str,
    encoding: PositionEncoding,
) -> Option<(usize, Range)> {
    let line = declaration_line(text, start_line, end_line, name)?;
    Some((line, name_range(text, line, name, encoding)?))
}

/// Returns a 1-based line of `text` without its line break.
fn line_content(text: &str, line: usize) -> Option<&str> {
    text.split_inclusive('\n')
        .nth(line.saturating_sub(1))
        .map(|line| line.trim_end_matches(['\n', '\r']))
}

/// Measures `text` in the units the client negotiated.
fn width(text: &str, encoding: PositionEncoding) -> u32 {
    let units = match encoding {
        PositionEncoding::Utf8 => text.len(),
        PositionEncoding::Utf16 => text.chars().map(char::len_utf16).sum(),
        PositionEncoding::Utf32 => text.chars().count(),
    };
    u32::try_from(units).unwrap_or(u32::MAX)
}

/// Finds `needle` in `haystack` where it is not part of a longer name.
fn whole_word(haystack: &str, needle: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(offset) = haystack.get(from..)?.find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        if !before.is_some_and(is_name_char) && !after.is_some_and(is_name_char) {
            return Some(start);
        }
        from = start + haystack[start..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// Whether `c` can appear inside an identifier, in any language analyzed
/// here. `$` is included for JavaScript.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn cpp_language_and_health_pipeline() {
        assert_eq!(language_for("cpp", Path::new("untitled")), Some(LANG::Cpp));
        for extension in [
            "cpp", "cc", "cxx", "hpp", "hxx", "hh", "h", "ipp", "tpp", "inl", "cppm", "ccm", "cxxm",
        ] {
            assert_eq!(
                language_for("plaintext", Path::new(&format!("sample.{extension}"))),
                Some(LANG::Cpp)
            );
        }
        let report = analyze(
            LANG::Cpp,
            "int add(int a, int b) { return a + b; }".into(),
            Path::new("sample.cpp"),
            &HealthConfig::default(),
        )
        .unwrap();
        assert_eq!(report.functions.len(), 1);
        assert_eq!(report.functions[0].display_name(), "add");
        assert!(report.quality.is_finite());
    }

    #[test]
    fn the_language_id_decides_when_it_is_known() {
        assert_eq!(
            language_for("rust", Path::new("weird.txt")),
            Some(LANG::Rust)
        );
        assert_eq!(
            language_for("typescriptreact", Path::new("a.ts")),
            Some(LANG::Tsx)
        );
        assert_eq!(
            language_for("java", Path::new("weird.txt")),
            Some(LANG::Java)
        );
    }

    #[test]
    fn an_unknown_language_id_falls_back_to_the_extension() {
        assert_eq!(
            language_for("plaintext", Path::new("/tmp/a.rs")),
            Some(LANG::Rust)
        );
        assert_eq!(
            language_for("plaintext", Path::new("/tmp/A.java")),
            Some(LANG::Java)
        );
        assert_eq!(language_for("plaintext", Path::new("/tmp/a.txt")), None);
        assert_eq!(language_for("", Path::new("/tmp/README")), None);
    }

    #[test]
    fn a_declaration_is_found_past_the_lines_its_annotations_take() {
        let text = "@Deprecated\npublic class Legacy {\n    @Override\n    public int plain() {\n    }\n}\n";
        assert_eq!(declaration_line(text, 1, 6, "Legacy"), Some(2));
        assert_eq!(declaration_line(text, 3, 5, "plain"), Some(4));
    }

    #[test]
    fn a_declaration_on_the_first_line_is_still_found_there() {
        let text = "fn add(a: u32) -> u32 {\n    a\n}\n";
        assert_eq!(declaration_line(text, 1, 3, "add"), Some(1));
    }

    #[test]
    fn a_name_only_inside_an_annotation_is_not_a_declaration() {
        let text = "@SuppressWarnings(\"plain\")\npublic int other() {\n}\n";
        assert_eq!(declaration_line(text, 1, 3, "plain"), None);
    }

    #[test]
    fn a_name_that_is_nowhere_in_the_header_has_no_line() {
        let text = "const f = function () {\n    return 1;\n};\n";
        assert_eq!(declaration_line(text, 1, 3, "<anonymous>"), None);
        assert_eq!(declaration_line(text, 1, 3, ""), None);
    }

    #[test]
    fn a_declaration_range_covers_the_name_on_the_line_it_was_found() {
        let text = "@Override\npublic int plain() {\n}\n";
        let (line, range) =
            declaration_range(text, 1, 3, "plain", PositionEncoding::Utf16).unwrap();
        assert_eq!(line, 2);
        assert_eq!(range.start.line, 1, "0-based on the wire");
        assert_eq!(range.start.character, 11);
        assert_eq!(range.end.character, 16);
    }

    fn uri(raw: &str) -> Uri {
        Uri::from_str(raw).unwrap()
    }

    #[test]
    fn a_file_uri_becomes_its_path() {
        assert_eq!(
            uri_to_path(&uri("file:///home/dev/project/src/main.rs")),
            PathBuf::from("/home/dev/project/src/main.rs")
        );
    }

    #[test]
    fn escapes_in_a_uri_are_decoded() {
        assert_eq!(
            uri_to_path(&uri("file:///home/dev/my%20project/a.rs")),
            PathBuf::from("/home/dev/my project/a.rs")
        );
    }

    #[test]
    fn a_query_or_fragment_is_not_part_of_the_path() {
        assert_eq!(
            uri_to_path(&uri("file:///a/b.rs?v=2")),
            PathBuf::from("/a/b.rs")
        );
    }

    #[test]
    fn a_non_file_uri_still_yields_an_extension() {
        let path = uri_to_path(&uri("untitled:Untitled-1.py"));
        assert_eq!(
            language_for("plaintext", &path),
            Some(LANG::Python),
            "got {path:?}"
        );
    }

    #[test]
    fn a_line_range_covers_the_line_content_without_its_break() {
        let text = "fn main() {\n    let a = 1;\n}\n";
        let range = line_range(text, 1, PositionEncoding::Utf8);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, "fn main() {".len() as u32));
    }

    #[test]
    fn a_line_range_counts_in_the_negotiated_encoding() {
        // The emoji is one character, two UTF-16 units, four UTF-8 bytes;
        // the other nine characters are one of each.
        let text = "let \u{1F600} = 1;\n";
        let utf8 = line_range(text, 1, PositionEncoding::Utf8).end.character;
        let utf16 = line_range(text, 1, PositionEncoding::Utf16).end.character;
        let utf32 = line_range(text, 1, PositionEncoding::Utf32).end.character;
        assert_eq!(utf8, 13);
        assert_eq!(utf16, 11);
        assert_eq!(utf32, 10);
    }

    #[test]
    fn a_line_past_the_end_yields_an_empty_range() {
        let range = line_range("one line\n", 99, PositionEncoding::Utf8);
        assert_eq!(range.start, range.end);
    }

    fn name(text: &str, line: usize, name: &str) -> Option<(u32, u32)> {
        name_range(text, line, name, PositionEncoding::Utf16)
            .map(|range| (range.start.character, range.end.character))
    }

    #[test]
    fn a_name_range_covers_the_declared_name() {
        assert_eq!(name("fn add(a: u32) -> u32 {\n", 1, "add"), Some((3, 6)));
        assert_eq!(name("def total(values):\n", 1, "total"), Some((4, 9)));
        assert_eq!(
            name("    async function load($el) {\n", 1, "load"),
            Some((19, 23))
        );
    }

    #[test]
    fn a_name_inside_a_longer_word_is_not_the_declaration() {
        // `add` starts `addHandler` before it names the function.
        assert_eq!(
            name("const addHandler = function add() {\n", 1, "add"),
            Some((28, 31))
        );
        assert_eq!(name("fn adder(a: u32) {\n", 1, "add"), None);
        assert_eq!(name("fn my_add(a: u32) {\n", 1, "add"), None);
    }

    #[test]
    fn a_name_that_is_not_on_the_line_has_no_range() {
        assert_eq!(name("fn add(a: u32) {\n", 1, "other"), None);
        assert_eq!(name("fn add(a: u32) {\n", 1, ""), None);
        assert_eq!(name("fn add(a: u32) {\n", 99, "add"), None);
    }

    #[test]
    fn a_name_range_counts_in_the_negotiated_encoding() {
        let text = "let \u{1F600} = fn wide() {\n";
        let utf8 = name_range(text, 1, "wide", PositionEncoding::Utf8).unwrap();
        let utf16 = name_range(text, 1, "wide", PositionEncoding::Utf16).unwrap();
        assert_eq!(utf8.start.character, 14);
        assert_eq!(utf16.start.character, 12);
        assert_eq!(utf16.end.character - utf16.start.character, 4);
    }

    #[test]
    fn analyze_scores_a_parsed_file() {
        let report = analyze(
            LANG::Rust,
            "fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n".to_string(),
            Path::new("a.rs"),
            &HealthConfig::default(),
        )
        .unwrap();
        assert_eq!(report.functions.len(), 1);
        assert_eq!(report.functions[0].display_name(), "add");
    }
}
