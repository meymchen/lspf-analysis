//! Rendering a function's health for hover.

use lspf::PositionEncoding;
use lspf::types::{Position, Range};
use lspf_analysis_core::health::{FileHealth, FunctionHealth, Grade};

use crate::document::name_range;

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

/// Renders a function's numbers as Markdown.
///
/// One row per metric, grouped under the pillar it scores, so a reader sees
/// both the verdict and the measurement that produced it. The pillar's own
/// score is the worst of its rows, which is why it is not repeated.
pub fn render(function: &FunctionHealth) -> String {
    let mut out = format!(
        "**`{name}`** — quality **{quality:.0}%** ({grade})\n\
         \n\
         | pillar | metric | value | score |\n\
         | --- | --- | ---: | ---: |\n",
        name = function.display_name(),
        quality = function.quality,
        grade = function.grade,
    );
    for pillar in function.scores.pillars() {
        for (index, metric) in pillar.measures.iter().enumerate() {
            out.push_str(&format!(
                "| {label} | {metric} | {value:.0} | {score:.0}% ({grade}) |\n",
                // The pillar is named once, on the row of its first metric.
                label = if index == 0 { pillar.name } else { "" },
                metric = metric.name,
                value = metric.value,
                score = metric.score,
                grade = Grade::of(metric.score),
            ));
        }
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
        let markdown = render(at(&report, 1, 4).unwrap());
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
    fn a_pillar_is_named_once_however_many_metrics_it_has() {
        let report = report();
        let markdown = render(at(&report, 1, 4).unwrap());
        assert_eq!(
            markdown.matches("| control flow |").count(),
            1,
            "the second row of a pillar leaves its label empty:\n{markdown}"
        );
    }
}
