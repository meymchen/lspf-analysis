//! Rendering a function's health for hover.

use lspf_analysis_core::health::{FileHealth, FunctionHealth, Grade};

/// Finds the function whose span covers a 1-based line.
///
/// Functions nest, so the innermost match wins: hovering inside a closure
/// should describe the closure, not the function containing it.
pub fn function_at(report: &FileHealth, line: usize) -> Option<&FunctionHealth> {
    report
        .functions
        .iter()
        .filter(|function| function.start_line <= line && line <= function.end_line)
        .max_by_key(|function| function.start_line)
}

/// Renders a function's four numbers as Markdown.
pub fn render(function: &FunctionHealth) -> String {
    let scores = &function.scores;
    format!(
        "**`{name}`** — quality **{quality:.0}%** ({grade})\n\
         \n\
         | metric | value | score |\n\
         | --- | ---: | ---: |\n\
         | complexity | {complexity:.0} | {complexity_score:.0}% ({complexity_grade}) |\n\
         | method length | {length:.0} | {length_score:.0}% ({length_grade}) |\n\
         | working memory | {memory:.0} | {memory_score:.0}% ({memory_grade}) |\n",
        name = function.display_name(),
        quality = function.quality,
        grade = function.grade,
        complexity = scores.complexity,
        complexity_score = scores.complexity_score,
        complexity_grade = Grade::of(scores.complexity_score),
        length = scores.length,
        length_score = scores.length_score,
        length_grade = Grade::of(scores.length_score),
        memory = scores.working_memory,
        memory_score = scores.working_memory_score,
        memory_grade = Grade::of(scores.working_memory_score),
    )
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

    #[test]
    fn a_line_outside_every_function_matches_nothing() {
        assert!(function_at(&report(), 5).is_none());
    }

    #[test]
    fn the_signature_line_matches_its_function() {
        let report = report();
        assert_eq!(function_at(&report, 1).unwrap().display_name(), "outer");
        assert_eq!(function_at(&report, 6).unwrap().display_name(), "other");
    }

    #[test]
    fn the_innermost_function_wins() {
        let report = report();
        // Line 2 is inside `outer` and is also the closure's own span.
        let found = function_at(&report, 2).unwrap();
        assert!(found.start_line >= 2, "expected the closure, got {found:?}");
    }

    #[test]
    fn rendering_names_all_four_numbers() {
        let report = report();
        let markdown = render(function_at(&report, 1).unwrap());
        assert!(markdown.contains("quality"));
        assert!(markdown.contains("complexity"));
        assert!(markdown.contains("method length"));
        assert!(markdown.contains("working memory"));
        assert!(markdown.contains("outer"));
    }
}
