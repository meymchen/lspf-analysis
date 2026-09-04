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
        "rust" => Some(LANG::Rust),
        "python" => Some(LANG::Python),
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
    let space = get_function_spaces(&language, text.into_bytes(), path)?;
    Some(file_health(&space, path, config))
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
    let index = line.saturating_sub(1);
    let content = text
        .split_inclusive('\n')
        .nth(index)
        .map(|line| line.trim_end_matches(['\n', '\r']))
        .unwrap_or("");
    let end = match encoding {
        PositionEncoding::Utf8 => content.len(),
        PositionEncoding::Utf16 => content.chars().map(char::len_utf16).sum(),
        PositionEncoding::Utf32 => content.chars().count(),
    };
    let line = u32::try_from(index).unwrap_or(u32::MAX);
    Range::new(
        Position::new(line, 0),
        Position::new(line, u32::try_from(end).unwrap_or(u32::MAX)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
    }

    #[test]
    fn an_unknown_language_id_falls_back_to_the_extension() {
        assert_eq!(
            language_for("plaintext", Path::new("/tmp/a.rs")),
            Some(LANG::Rust)
        );
        assert_eq!(language_for("plaintext", Path::new("/tmp/a.txt")), None);
        assert_eq!(language_for("", Path::new("/tmp/README")), None);
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
