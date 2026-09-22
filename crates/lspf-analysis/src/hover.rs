//! Rendering a function's or a class's health for hover.

use lspf::PositionEncoding;
use lspf::types::{ClientCapabilities, Position, Range};
use lspf_analysis_core::health::{ClassHealth, FileHealth, FunctionHealth, Grade, Pillar};

use crate::document::declaration_range;
use crate::i18n::Locale;
use crate::theme::{Palette, Theme};

/// Finds the function whose declared name is under `position`.
///
/// These metrics supplement whatever else the editor knows about the symbol,
/// so they are offered only on the name itself. Answering for every line of
/// a function would put a table in front of the reader whenever they asked
/// about a local variable, which is not what they asked.
///
/// The candidates are the spaces whose lines enclose `position`, rather than
/// only those starting on it: a Java method's space starts at its first
/// annotation, so `@Override` is the space's line and the name is on the
/// next one. [`declaration_range`] finds which line actually declares it.
///
/// Functions can nest on one line, so the tightest span wins.
pub fn function_at<'a>(
    report: &'a FileHealth,
    text: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Option<(&'a FunctionHealth, Range)> {
    let line = position.line as usize + 1;
    let mut found: Option<(&FunctionHealth, Range)> = None;
    for function in report
        .functions
        .iter()
        .filter(|f| f.start_line <= line && line <= f.end_line)
    {
        let Some(name) = function.name.as_deref() else {
            continue;
        };
        let Some((declared_on, range)) =
            declaration_range(text, function.start_line, function.end_line, name, encoding)
        else {
            continue;
        };
        if declared_on != line {
            continue;
        }
        if position.character < range.start.character || position.character >= range.end.character {
            continue;
        }
        let tighter = found
            .as_ref()
            .is_none_or(|(current, _)| function.end_line < current.end_line);
        if tighter {
            found = Some((function, range));
        }
    }
    found
}

/// Finds the class or interface whose declared name is under `position`.
///
/// The same rule as [`function_at`]: the name, and nowhere else. Classes do
/// not nest on one line the way functions do, so the first match wins.
pub fn class_at<'a>(
    report: &'a FileHealth,
    text: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Option<(&'a ClassHealth, Range)> {
    let line = position.line as usize + 1;
    report
        .classes
        .iter()
        .filter(|class| class.start_line <= line && line <= class.end_line)
        .find_map(|class| {
            let name = class.name.as_deref()?;
            let (declared_on, range) =
                declaration_range(text, class.start_line, class.end_line, name, encoding)?;
            (declared_on == line
                && position.character >= range.start.character
                && position.character < range.end.character)
                .then_some((class, range))
        })
}

/// The letter standing in for a grade in the table's leading column.
///
/// A to D rather than the initial of the grade word: the words are
/// translated and their initials are not, and a reader who has met a letter
/// grade anywhere already knows which end of the scale A is on.
fn letter(grade: Grade) -> &'static str {
    match grade {
        Grade::Excellent => "A",
        Grade::Good => "B",
        Grade::Fair => "C",
        Grade::Poor => "D",
    }
}

/// How a grade's letter is drawn, once the client and its theme are known.
///
/// Two things have to line up before a letter can be coloured, and they
/// arrive from different places and at different times. Whether the client
/// renders a `<span>` at all is a capability, settled once at `initialize`:
/// LSP 3.17 has a client list the HTML tags its Markdown renderer keeps, in
/// `general.markdown.allowedTags`, and a client that renders neither the tag
/// nor its contents would show the markup around the letter, which is worse
/// than a bare letter. Which colour to use then depends on the theme, which
/// rides in the settings and changes whenever the reader changes it.
///
/// [`Bare`](Self::Bare) is the answer to three separate questions — the
/// client keeps no spans, the reader is on a high contrast theme, or the
/// palette could not be built — so the rendering code only ever asks one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Colour {
    /// The client keeps `span`, and this is what to draw with.
    Spans(Palette),
    /// Letters go out on their own.
    #[default]
    Bare,
}

impl Colour {
    /// Decides from what the client can render and what it is rendering on.
    ///
    /// The client may declare its kept tags in either of two places, and both
    /// are read: `general.markdown.allowedTags` in its capabilities, which is
    /// the standard one, and `lspfAnalysis.markdown.allowedTags` in its
    /// settings, which exists for a client whose LSP stack will not let it
    /// send the first. They are the same claim, so they are unioned.
    ///
    /// A client that reported no theme gets [`Palette::untold`] rather than
    /// nothing: it can still show a colour, it just cannot be fitted to a
    /// surface nobody named.
    pub fn of(
        capabilities: &ClientCapabilities,
        declared: Option<&[String]>,
        theme: Option<&Theme>,
    ) -> Self {
        let advertised = capabilities
            .general
            .as_ref()
            .and_then(|general| general.markdown.as_ref())
            .and_then(|markdown| markdown.allowed_tags.as_deref())
            .unwrap_or_default();
        let keeps_spans = advertised
            .iter()
            .chain(declared.unwrap_or_default())
            .any(|tag| tag.eq_ignore_ascii_case("span"));
        if !keeps_spans {
            return Self::Bare;
        }
        match theme {
            None => Self::Spans(Palette::untold()),
            Some(theme) => Palette::for_theme(theme).map_or(Self::Bare, Self::Spans),
        }
    }

    /// Draws one grade for the table's leading cell.
    fn cell(&self, grade: Grade) -> String {
        match self {
            Self::Spans(palette) => format!(
                "<span style=\"color:{};\">{}</span>",
                palette.of(grade).hex(),
                letter(grade)
            ),
            Self::Bare => letter(grade).to_string(),
        }
    }
}

/// Renders a function's numbers as Markdown, in the reader's language.
///
/// One row per metric, grouped under the pillar it scores, so a reader sees
/// both the verdict and the measurement that produced it. The pillar's own
/// score is the worst of its rows, which is why it is not repeated.
///
/// Beyond the one `span` the client asked for, the Markdown is portable — no
/// other HTML, no editor-specific icon syntax — because every LSP client
/// renders this. A client that can do more is free to add to it; the VS Code
/// extension appends its own footer.
pub fn render(function: &FunctionHealth, locale: Locale, colour: &Colour) -> String {
    render_pillars(
        function.display_name(),
        function.quality,
        function.grade,
        &function.scores.pillars(),
        function.scores.worst_pillar(),
        locale,
        colour,
    )
}

/// Renders a class's numbers the same way, from its one pillar.
pub fn render_class(class: &ClassHealth, locale: Locale, colour: &Colour) -> String {
    let pillar = &class.scores.class_design;
    render_pillars(
        class.display_name(),
        class.quality,
        class.grade,
        &[pillar],
        pillar,
        locale,
        colour,
    )
}

/// The shared body: a heading, one table row per measure, and a sentence
/// naming the measure that set the verdict.
fn render_pillars(
    name: &str,
    quality: f64,
    grade: Grade,
    pillars: &[&Pillar],
    worst: &Pillar,
    locale: Locale,
    colour: &Colour,
) -> String {
    // No bar under the heading: it drew the percentage standing beside it a
    // second way and said nothing the letters down the table do not.
    let mut out = format!(
        "**`{name}`**  ·  {quality_label} **{quality:.0}%**  ·  {grade}\n\
         \n\
         | {grade_label} | {pillar} | {metric} | {value} |\n\
         | :-: | :-- | :-- | --: |\n",
        quality_label = locale.t("quality"),
        grade = locale.t(grade.as_str()),
        grade_label = locale.t("grade"),
        pillar = locale.t("pillar"),
        metric = locale.t("metric"),
        value = locale.t("value"),
    );
    for pillar in pillars {
        for (index, metric) in pillar.measures.iter().enumerate() {
            out.push_str(&format!(
                "| {mark} | {label} | {metric} | {value:.0} / {threshold:.0} |\n",
                mark = colour.cell(Grade::of(metric.score)),
                // The pillar is named once, on the row of its first metric.
                label = if index == 0 {
                    locale.t(pillar.name)
                } else {
                    ""
                },
                metric = locale.t(metric.name),
                value = metric.value,
                threshold = metric.threshold,
            ));
        }
    }
    if let Some(metric) = worst.worst_measure() {
        out.push('\n');
        out.push_str(&locale.fill(
            "Weakest: **{0}** — {1} {2} against a threshold of {3}, scoring {4}% ({5}).",
            &[
                locale.t(worst.name),
                locale.t(metric.name),
                &format!("{:.0}", metric.value),
                &format!("{:.0}", metric.threshold),
                &format!("{:.0}", metric.score),
                locale.t(Grade::of(metric.score).as_str()),
            ],
        ));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::analyze;
    use crate::theme::{Rgb, ThemeKind};
    use lspf::types::{GeneralClientCapabilities, MarkdownClientCapabilities};
    use lspf_analysis_core::LANG;
    use lspf_analysis_core::health::HealthConfig;
    use std::path::Path;

    const SOURCE: &str = "fn outer(a: u32) -> u32 {
    let f = |x: u32| x + a;
    f(a)
}

fn other() -> u32 {
    1
}
";

    fn report() -> FileHealth {
        analyze(
            LANG::Rust,
            SOURCE.to_string(),
            Path::new("a.rs"),
            &HealthConfig::default(),
        )
        .unwrap()
    }

    /// Looks up a 1-based line and 0-based column, as an editor would.
    fn at(report: &FileHealth, line: u32, character: u32) -> Option<&FunctionHealth> {
        function_at(
            report,
            SOURCE,
            Position::new(line - 1, character),
            PositionEncoding::Utf16,
        )
        .map(|(function, _)| function)
    }

    #[test]
    fn the_name_matches_its_function() {
        let report = report();
        // `fn outer(...)`: the name starts at column 3.
        assert_eq!(at(&report, 1, 3).unwrap().display_name(), "outer");
        assert_eq!(at(&report, 1, 7).unwrap().display_name(), "outer");
        assert_eq!(at(&report, 6, 3).unwrap().display_name(), "other");
    }

    #[test]
    fn the_rest_of_the_signature_matches_nothing() {
        let report = report();
        assert!(at(&report, 1, 0).is_none(), "the `fn` keyword");
        assert!(at(&report, 1, 8).is_none(), "the opening parenthesis");
        assert!(at(&report, 1, 10).is_none(), "the parameter");
    }

    #[test]
    fn a_line_inside_a_function_matches_nothing() {
        let report = report();
        // Hovering a local must leave the editor's own hover alone.
        assert!(at(&report, 3, 4).is_none());
        assert!(at(&report, 5, 0).is_none());
    }

    #[test]
    fn an_anonymous_function_has_no_name_to_hover() {
        let report = report();
        // Line 2 is the closure's span, but a closure has no name of its own.
        assert!(at(&report, 2, 8).is_none());
    }

    #[test]
    fn the_range_covers_exactly_the_name() {
        let report = report();
        let (_, range) = function_at(
            &report,
            SOURCE,
            Position::new(0, 4),
            PositionEncoding::Utf16,
        )
        .unwrap();
        assert_eq!(range.start, Position::new(0, 3));
        assert_eq!(range.end, Position::new(0, 8));
    }

    #[test]
    fn rendering_names_every_pillar_and_metric() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English, &Colour::Bare);
        for expected in [
            "outer",
            "quality",
            "control flow",
            "cognitive complexity",
            "cyclomatic complexity",
            "size",
            "statements",
            "vocabulary load",
            "working memory",
            "Halstead difficulty",
            "interface",
            "parameters",
        ] {
            assert!(
                markdown.contains(expected),
                "{expected} missing:\n{markdown}"
            );
        }
    }

    #[test]
    fn a_chinese_reader_gets_the_vocabulary_and_the_sentences() {
        let report = report();
        let markdown = render(
            at(&report, 1, 4).unwrap(),
            Locale::SimplifiedChinese,
            &Colour::Bare,
        );
        for expected in [
            "质量",
            "支柱",
            "指标",
            "控制流",
            "认知复杂度",
            "规模",
            "语句数",
            "词汇负担",
            "工作记忆",
            "接口",
            "参数个数",
            "等级",
            "最弱：",
        ] {
            assert!(
                markdown.contains(expected),
                "{expected} missing:\n{markdown}"
            );
        }
        assert!(
            !markdown.contains("cognitive complexity"),
            "nothing is left half-translated:\n{markdown}"
        );
        // The function's own name is the reader's, not ours to translate.
        assert!(markdown.contains("outer"), "{markdown}");
    }

    #[test]
    fn no_bar_is_drawn_anywhere_in_a_hover() {
        // A bar per row repeated the percentage beside it and took ten
        // columns to do it, which is what pushed the table into wrapping;
        // the one under the heading then said the same as the heading.
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English, &Colour::Bare);
        for cell in ['█', '▉', '▊', '▋', '▌', '▍', '▎', '▏', '░'] {
            assert!(!markdown.contains(cell), "{cell} drawn in:\n{markdown}");
        }
    }

    #[test]
    fn the_heading_carries_the_quality_score() {
        let report = report();
        let function = at(&report, 1, 4).unwrap();
        let markdown = render(function, Locale::English, &Colour::Bare);
        let heading = markdown.lines().next().unwrap();
        assert!(heading.contains("**`outer`**"), "{heading}");
        assert!(
            heading.contains(&format!("**{:.0}%**", function.quality)),
            "{heading}"
        );
        // The heading, a blank line, then the table: nothing between them.
        assert_eq!(markdown.lines().nth(1), Some(""), "{markdown}");
        assert!(
            markdown.lines().nth(2).unwrap().starts_with("| "),
            "{markdown}"
        );
    }

    #[test]
    fn every_row_is_marked_with_the_grade_of_its_metric() {
        let report = report();
        let function = at(&report, 1, 4).unwrap();
        let markdown = render(function, Locale::English, &Colour::Bare);
        let rows: Vec<&str> = markdown
            .lines()
            .filter(|line| line.starts_with("| ") && !line.contains(":--"))
            .collect();
        // The header, whose leading cell is empty, and one row per measure.
        let measures: usize = function
            .scores
            .pillars()
            .iter()
            .map(|p| p.measures.len())
            .sum();
        assert_eq!(rows.len(), measures + 1, "{markdown}");
        for row in &rows[1..] {
            let leading = row.split('|').nth(1).unwrap().trim();
            assert!(
                ["A", "B", "C", "D"].contains(&leading),
                "row does not lead with a grade letter: {row}"
            );
        }
    }

    #[test]
    fn a_client_that_keeps_spans_gets_its_letters_coloured() {
        let report = report();
        let markdown = render(
            at(&report, 1, 4).unwrap(),
            Locale::English,
            &Colour::Spans(Palette::untold()),
        );
        assert!(
            markdown.contains("| <span style=\"color:#2f9e44;\">A</span> | control flow |"),
            "{markdown}"
        );
        // The colour rides on the letter and nowhere else: anything more
        // would be markup a client that strips the tag turns into nothing.
        assert_eq!(markdown.matches("<span").count(), 6, "{markdown}");
        assert_eq!(markdown.matches("</span>").count(), 6, "{markdown}");
    }

    #[test]
    fn a_client_that_says_nothing_gets_bare_letters() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English, &Colour::Bare);
        assert!(markdown.contains("| A | control flow |"), "{markdown}");
        assert!(
            !markdown.contains('<'),
            "a client that renders no HTML must not be shown any:\n{markdown}"
        );
    }

    /// Capabilities listing exactly the tags given.
    fn listing(tags: Option<Vec<String>>) -> ClientCapabilities {
        ClientCapabilities {
            general: Some(GeneralClientCapabilities {
                markdown: Some(MarkdownClientCapabilities {
                    parser: "marked".into(),
                    version: None,
                    allowed_tags: tags,
                }),
                ..GeneralClientCapabilities::default()
            }),
            ..ClientCapabilities::default()
        }
    }

    #[test]
    fn colour_follows_the_tags_the_client_listed() {
        let with = |tags: Option<Vec<String>>| Colour::of(&listing(tags), None, None);
        let spans = Colour::Spans(Palette::untold());
        assert_eq!(with(Some(vec!["span".into()])), spans);
        assert_eq!(with(Some(vec!["p".into(), "SPAN".into()])), spans);
        assert_eq!(with(Some(vec!["p".into(), "img".into()])), Colour::Bare);
        assert_eq!(with(Some(vec![])), Colour::Bare);
        assert_eq!(with(None), Colour::Bare);
        // A client that sent no capabilities at all is not offered markup.
        assert_eq!(
            Colour::of(&ClientCapabilities::default(), None, None),
            Colour::Bare
        );
    }

    #[test]
    fn a_client_may_declare_its_tags_in_its_settings_instead() {
        // Visual Studio's LSP stack sends `initialize` itself and offers no
        // hook for shaping capabilities, so the tag list has to arrive the
        // other way. Without this it gets bare letters however it is themed.
        let silent = ClientCapabilities::default();
        let declared = ["span".to_string()];
        assert_eq!(
            Colour::of(&silent, Some(&declared), None),
            Colour::Spans(Palette::untold())
        );
        // The two sources are the same claim: saying it twice says it once.
        let both = listing(Some(vec!["span".into()]));
        assert_eq!(
            Colour::of(&both, Some(&declared), None),
            Colour::of(&both, None, None)
        );
        // And a list that does not name `span` still grants nothing.
        let other = ["p".to_string(), "img".to_string()];
        assert_eq!(Colour::of(&silent, Some(&other), None), Colour::Bare);
        assert_eq!(Colour::of(&silent, Some(&[]), None), Colour::Bare);
    }

    #[test]
    fn a_client_that_reported_no_theme_keeps_the_palette_it_always_had() {
        // Plain LSP clients — nvim, Helix, Emacs — say nothing about their
        // appearance, and must see exactly what they saw before themes were
        // ever sent.
        let spans = listing(Some(vec!["span".into()]));
        assert_eq!(
            Colour::of(&spans, None, None),
            Colour::Spans(Palette::untold()),
            "a client that named no theme is not fitted to a guess"
        );
    }

    #[test]
    fn a_theme_picks_the_palette_and_high_contrast_picks_none() {
        let spans = listing(Some(vec!["span".into()]));
        let themed = |kind| {
            Colour::of(
                &spans,
                None,
                Some(&Theme {
                    kind,
                    background: None,
                }),
            )
        };
        // A reader who asked not to be told things by hue is told by letter.
        assert_eq!(themed(ThemeKind::HighContrast), Colour::Bare);
        // The two ordinary kinds differ from each other and from the palette
        // sent to a client that named no theme at all.
        let light = themed(ThemeKind::Light);
        let dark = themed(ThemeKind::Dark);
        assert_ne!(light, dark);
        assert_ne!(light, Colour::Spans(Palette::untold()));
        assert_ne!(dark, Colour::Spans(Palette::untold()));
    }

    #[test]
    fn a_theme_reaches_the_letters_a_hover_draws() {
        // The end of the wire: a surface the client reported changes the hex
        // that comes out in the Markdown.
        let report = report();
        let spans = listing(Some(vec!["span".into()]));
        let on = |background| {
            render(
                at(&report, 1, 4).unwrap(),
                Locale::English,
                &Colour::of(
                    &spans,
                    None,
                    Some(&Theme {
                        kind: ThemeKind::Dark,
                        background: Rgb::parse(background),
                    }),
                ),
            )
        };
        // The stock dark surface leaves the base green untouched.
        assert!(
            on("#1f1f1f").contains("color:#30d158;"),
            "{}",
            on("#1f1f1f")
        );
        // A surface bright enough to swallow it does not.
        assert!(
            !on("#d8d8d8").contains("color:#30d158;"),
            "{}",
            on("#d8d8d8")
        );
    }

    #[test]
    fn every_metric_is_shown_against_its_threshold() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English, &Colour::Bare);
        // The defaults, so a reader can see what a number is judged against.
        assert!(markdown.contains("/ 15"), "{markdown}");
        assert!(markdown.contains("/ 10"), "{markdown}");
        assert!(markdown.contains("/ 30"), "{markdown}");
        assert!(markdown.contains("/ 4"), "{markdown}");
    }

    #[test]
    fn the_weakest_pillar_is_called_out_under_the_table() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English, &Colour::Bare);
        let weakest = markdown
            .lines()
            .find(|line| line.starts_with("Weakest:"))
            .unwrap_or_else(|| panic!("no weakest line:\n{markdown}"));
        let worst = at(&report, 1, 4).unwrap().scores.worst_pillar();
        assert!(weakest.contains(worst.name), "{weakest}");
    }

    #[test]
    fn a_pillar_is_named_once_however_many_metrics_it_has() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English, &Colour::Bare);
        assert_eq!(
            markdown.matches("| control flow |").count(),
            1,
            "the second row of a pillar leaves its label empty:\n{markdown}"
        );
    }

    const JAVA: &str = "public class Box {
    public int side;

    public int area() {
        return side * side;
    }
}
";

    fn java_report() -> FileHealth {
        analyze(
            LANG::Java,
            JAVA.to_string(),
            Path::new("Box.java"),
            &HealthConfig::default(),
        )
        .unwrap()
    }

    fn class_at_position(report: &FileHealth, line: u32, character: u32) -> Option<&ClassHealth> {
        class_at(
            report,
            JAVA,
            Position::new(line - 1, character),
            PositionEncoding::Utf16,
        )
        .map(|(class, _)| class)
    }

    #[test]
    fn a_class_name_matches_its_class() {
        let report = java_report();
        // `public class Box`: the name starts at column 13.
        assert_eq!(
            class_at_position(&report, 1, 13).unwrap().name.as_deref(),
            Some("Box")
        );
        assert!(
            class_at_position(&report, 1, 0).is_none(),
            "the `public` keyword"
        );
        assert!(class_at_position(&report, 4, 15).is_none(), "a method name");
    }

    #[test]
    fn a_class_renders_its_three_measures() {
        let report = java_report();
        let markdown = render_class(
            class_at_position(&report, 1, 13).unwrap(),
            Locale::English,
            &Colour::Bare,
        );
        assert!(markdown.contains("**`Box`**"), "{markdown}");
        assert!(
            markdown.contains("| class design | weighted methods |"),
            "{markdown}"
        );
        assert!(markdown.contains("public methods"), "{markdown}");
        assert!(markdown.contains("public attributes"), "{markdown}");
        // The defaults, so a reader can see what a number is judged against.
        assert!(markdown.contains("/ 34"), "{markdown}");
        assert!(markdown.contains("/ 14"), "{markdown}");
        assert!(markdown.contains("/ 8"), "{markdown}");
    }

    /// Java folds annotations and modifiers into the declaration node, so
    /// every space here starts a line or more above its own name.
    const ANNOTATED: &str = "@Deprecated
public class Legacy {
    @Override
    @SuppressWarnings(\"unchecked\")
    public int plain(int a, int b) {
        return a + b;
    }
}
";

    fn annotated_report() -> FileHealth {
        analyze(
            LANG::Java,
            ANNOTATED.to_string(),
            Path::new("Legacy.java"),
            &HealthConfig::default(),
        )
        .unwrap()
    }

    #[test]
    fn an_annotated_method_hovers_on_its_signature_line() {
        let report = annotated_report();
        let found = |line: u32, character: u32| {
            function_at(
                &report,
                ANNOTATED,
                Position::new(line - 1, character),
                PositionEncoding::Utf16,
            )
            .map(|(function, _)| function.display_name().to_string())
        };
        // `    public int plain(int a, int b) {`: the name starts at column 15.
        assert_eq!(found(5, 15).as_deref(), Some("plain"));
        assert_eq!(found(5, 19).as_deref(), Some("plain"));
        // The annotations above it are not the declaration.
        assert_eq!(found(3, 4), None, "@Override");
        assert_eq!(found(4, 4), None, "@SuppressWarnings");
        // Nor is the rest of the signature, or the body.
        assert_eq!(found(5, 4), None, "the `public` keyword");
        assert_eq!(found(6, 8), None, "a line inside the body");
    }

    #[test]
    fn an_annotated_class_hovers_on_its_declaration_line() {
        let report = annotated_report();
        let found = |line: u32, character: u32| {
            class_at(
                &report,
                ANNOTATED,
                Position::new(line - 1, character),
                PositionEncoding::Utf16,
            )
            .map(|(class, _)| class.display_name().to_string())
        };
        // `public class Legacy {`: the name starts at column 13.
        assert_eq!(found(2, 13).as_deref(), Some("Legacy"));
        assert_eq!(found(1, 0), None, "@Deprecated");
        assert_eq!(found(2, 0), None, "the `public` keyword");
    }

    #[test]
    fn a_name_inside_an_annotation_does_not_stand_in_for_the_declaration() {
        // A whole-word search would otherwise find `plain` in the
        // annotation's string and answer on the wrong line.
        let source = "public class C {
    @SuppressWarnings(\"plain\")
    public int plain(int a) {
        return a;
    }
}
";
        let report = analyze(
            LANG::Java,
            source.to_string(),
            Path::new("C.java"),
            &HealthConfig::default(),
        )
        .unwrap();
        let found = |line: u32, character: u32| {
            function_at(
                &report,
                source,
                Position::new(line - 1, character),
                PositionEncoding::Utf16,
            )
            .map(|(function, _)| function.display_name().to_string())
        };
        assert_eq!(found(3, 15).as_deref(), Some("plain"), "the declaration");
        assert_eq!(found(2, 23), None, "the string inside the annotation");
    }
}
