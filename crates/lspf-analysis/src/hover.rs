//! Rendering a function's health for hover.

use lspf::PositionEncoding;
use lspf::types::{Position, Range};
use lspf_analysis_core::health::{FileHealth, FunctionHealth, Grade};

use crate::document::name_range;
use crate::i18n::Locale;

/// Finds the function whose declared name is under `position`.
///
/// These metrics supplement whatever else the editor knows about the symbol,
/// so they are offered only on the name itself. Answering for every line of
/// a function would put a table in front of the reader whenever they asked
/// about a local variable, which is not what they asked.
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
    for function in report.functions.iter().filter(|f| f.start_line == line) {
        let Some(name) = function.name.as_deref() else {
            continue;
        };
        let Some(range) = name_range(text, line, name, encoding) else {
            continue;
        };
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

/// How many cells a score bar is drawn with.
const BAR_CELLS: usize = 10;

/// Draws a 0-100 score as a bar.
///
/// Block characters rather than an image or a codicon: a hover is Markdown,
/// and every editor that can show Markdown can show these. A reader scans
/// the column of bars far faster than a column of numbers.
///
/// These are never wrapped in a code span. VS Code draws inline code with a
/// background and horizontal padding, which puts a gap on either side of
/// every bar and breaks up the column. The two glyphs come from the same
/// Unicode block, which is designed to tile, so they line up without one.
fn bar(score: f64) -> String {
    let filled = ((score / 100.0) * BAR_CELLS as f64)
        .round()
        .clamp(0.0, BAR_CELLS as f64) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(BAR_CELLS - filled))
}

/// Renders a function's numbers as Markdown, in the reader's language.
///
/// One row per metric, grouped under the pillar it scores, so a reader sees
/// both the verdict and the measurement that produced it. The pillar's own
/// score is the worst of its rows, which is why it is not repeated.
///
/// The Markdown is deliberately portable — no HTML, no editor-specific icon
/// syntax — because every LSP client renders this. A client that can do
/// more is free to add to it; the VS Code extension appends its own footer.
pub fn render(function: &FunctionHealth, locale: Locale) -> String {
    let worst = function.scores.worst_pillar();
    let mut out = format!(
        "**`{name}`**  ·  {quality_label} **{quality:.0}%**  ·  {grade}\n\
         \n\
         {overall}\n\
         \n\
         | {pillar} | {metric} | {value} | {score} |\n\
         | :-- | :-- | --: | :-- |\n",
        name = function.display_name(),
        quality_label = locale.t("quality"),
        quality = function.quality,
        grade = locale.t(function.grade.as_str()),
        overall = bar(function.quality),
        pillar = locale.t("pillar"),
        metric = locale.t("metric"),
        value = locale.t("value"),
        score = locale.t("score"),
    );
    for pillar in function.scores.pillars() {
        for (index, metric) in pillar.measures.iter().enumerate() {
            out.push_str(&format!(
                "| {label} | {metric} | {value:.0} / {threshold:.0} | {bar} {score:.0}% |\n",
                // The pillar is named once, on the row of its first metric.
                label = if index == 0 {
                    locale.t(pillar.name)
                } else {
                    ""
                },
                metric = locale.t(metric.name),
                value = metric.value,
                threshold = metric.threshold,
                bar = bar(metric.score),
                score = metric.score,
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
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English);
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
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::SimplifiedChinese);
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
    fn a_bar_fills_in_proportion_to_the_score() {
        assert_eq!(bar(0.0), "░░░░░░░░░░");
        assert_eq!(bar(100.0), "██████████");
        assert_eq!(bar(50.0), "█████░░░░░");
        // A score can only be in (0, 100], but the bar must not panic on
        // subtracting past zero if that ever stops being true.
        assert_eq!(bar(-20.0), "░░░░░░░░░░");
        assert_eq!(bar(140.0), "██████████");
        assert_eq!(bar(f64::NAN).chars().count(), BAR_CELLS);
    }

    #[test]
    fn a_bar_is_never_wrapped_in_a_code_span() {
        // VS Code draws inline code with a background and padding, which
        // would put a gap on either side of every bar.
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English);
        for line in markdown.lines() {
            assert!(
                !line.contains("`\u{2588}") && !line.contains("`\u{2591}"),
                "bar in a code span: {line}"
            );
            assert!(
                !line.contains("\u{2588}`") && !line.contains("\u{2591}`"),
                "bar in a code span: {line}"
            );
        }
    }

    #[test]
    fn every_metric_is_shown_against_its_threshold() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English);
        // The defaults, so a reader can see what a number is judged against.
        assert!(markdown.contains("/ 15"), "{markdown}");
        assert!(markdown.contains("/ 10"), "{markdown}");
        assert!(markdown.contains("/ 30"), "{markdown}");
        assert!(markdown.contains("/ 4"), "{markdown}");
    }

    #[test]
    fn the_weakest_pillar_is_called_out_under_the_table() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English);
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
        let markdown = render(at(&report, 1, 4).unwrap(), Locale::English);
        assert_eq!(
            markdown.matches("| control flow |").count(),
            1,
            "the second row of a pillar leaves its label empty:\n{markdown}"
        );
    }
}
