//! Turning a health report into LSP diagnostics.

use lspf::PositionEncoding;
use lspf::types::{Code, Diagnostic, DiagnosticSeverity};
use lspf_analysis_core::health::{FileHealth, FunctionHealth, HealthConfig};

use crate::config::Settings;
use crate::document::line_range;

/// The `source` every diagnostic this server publishes carries.
pub const SOURCE: &str = "lspf-analysis";

/// A pillar score at or below this is called out on its own, when
/// per-metric diagnostics are enabled. It is the point where a pillar has
/// passed its threshold and kept going.
const PILLAR_FLOOR: f64 = 50.0;

/// Builds the diagnostics for one analyzed file.
///
/// `text` is the source the report was computed from: the ranges are
/// resolved against it, so passing a different revision would misplace them.
pub fn build(
    report: &FileHealth,
    text: &str,
    encoding: PositionEncoding,
    settings: &Settings,
) -> Vec<Diagnostic> {
    if !settings.diagnostics.enabled {
        return Vec::new();
    }
    let config = &settings.health;
    let mut diagnostics = Vec::new();

    for function in &report.functions {
        if function.quality < config.quality_warn {
            diagnostics.push(quality_diagnostic(function, text, encoding, config));
        } else if settings.diagnostics.per_metric {
            diagnostics.extend(pillar_diagnostics(function, text, encoding));
        }
    }

    if settings.diagnostics.file && report.quality < config.quality_error {
        diagnostics.push(Diagnostic {
            range: line_range(text, 1, encoding),
            severity: Some(DiagnosticSeverity::Warning),
            code: Some(Code::String("file-quality".into())),
            source: Some(SOURCE.into()),
            message: format!(
                "file quality {quality:.0}% ({grade}) across {count} function{plural}",
                quality = report.quality,
                grade = report.grade,
                count = report.functions.len(),
                plural = if report.functions.len() == 1 { "" } else { "s" },
            )
            .into(),
            ..Diagnostic::default()
        });
    }

    diagnostics
}

/// The headline diagnostic: this function is below the quality threshold.
fn quality_diagnostic(
    function: &FunctionHealth,
    text: &str,
    encoding: PositionEncoding,
    config: &HealthConfig,
) -> Diagnostic {
    let severity = if function.quality < config.quality_error {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };
    let (pillar, _) = function.scores.worst_pillar();
    Diagnostic {
        range: line_range(text, function.start_line, encoding),
        severity: Some(severity),
        code: Some(Code::String("quality".into())),
        source: Some(SOURCE.into()),
        message: format!(
            "function `{name}`: quality {quality:.0}% ({grade}), worst pillar {pillar} \
             — complexity {complexity:.0}, length {length:.0} statements, working memory {memory:.0}",
            name = function.display_name(),
            quality = function.quality,
            grade = function.grade,
            complexity = function.scores.complexity,
            length = function.scores.length,
            memory = function.scores.working_memory,
        )
        .into(),
        ..Diagnostic::default()
    }
}

/// Advisory diagnostics for a function that passes overall but has one
/// pillar sitting well past its threshold.
fn pillar_diagnostics(
    function: &FunctionHealth,
    text: &str,
    encoding: PositionEncoding,
) -> Vec<Diagnostic> {
    let scores = &function.scores;
    let pillars = [
        (
            "complexity",
            scores.complexity_score,
            scores.complexity,
            "cognitive complexity",
        ),
        (
            "method-length",
            scores.length_score,
            scores.length,
            "statements",
        ),
        (
            "working-memory",
            scores.working_memory_score,
            scores.working_memory,
            "names held at once",
        ),
    ];

    pillars
        .into_iter()
        .filter(|(_, score, _, _)| *score < PILLAR_FLOOR)
        .map(|(code, _, raw, label)| Diagnostic {
            range: line_range(text, function.start_line, encoding),
            severity: Some(DiagnosticSeverity::Information),
            code: Some(Code::String(code.into())),
            source: Some(SOURCE.into()),
            message: format!(
                "function `{name}`: {raw:.0} {label}",
                name = function.display_name(),
            )
            .into(),
            ..Diagnostic::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::analyze;
    use lspf_analysis_core::LANG;
    use std::path::Path;

    const SIMPLE: &str = "fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";

    /// A function bad enough on all three pillars to fail the shipped
    /// threshold: six four-deep nests over a wide signature.
    fn tangled() -> String {
        let mut source =
            String::from("fn tangled(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 {\n");
        source.push_str("    let mut total = 0;\n");
        for index in 0..6 {
            source.push_str(&format!(
                "    if a > {index} {{
        if b > {index} {{
            if c > {index} {{
                if d > {index} {{
                    let step{index} = a + b + c + d + e;
                    total = total + step{index};
                }}
            }}
        }}
    }}\n"
            ));
        }
        source.push_str("    total\n}\n");
        source
    }

    fn diagnostics_for(source: &str, settings: &Settings) -> Vec<Diagnostic> {
        let path = Path::new("a.rs");
        let report = analyze(LANG::Rust, source.to_string(), path, &settings.health).unwrap();
        build(&report, source, PositionEncoding::Utf16, settings)
    }

    fn message(diagnostic: &Diagnostic) -> String {
        match &diagnostic.message {
            lspf::types::Message::String(text) => text.clone(),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn a_healthy_function_produces_nothing() {
        assert!(diagnostics_for(SIMPLE, &Settings::default()).is_empty());
    }

    #[test]
    fn a_tangled_function_is_reported_on_its_signature_line() {
        let diagnostics = diagnostics_for(&tangled(), &Settings::default());
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.range.start.line, 0);
        assert_eq!(diagnostic.source.as_deref(), Some(SOURCE));
        assert_eq!(diagnostic.code, Some(Code::String("quality".into())));
        assert!(message(diagnostic).contains("tangled"), "{diagnostic:?}");
    }

    #[test]
    fn the_threshold_decides_what_is_reported() {
        let mut settings = Settings::default();
        settings.health.quality_warn = 100.0;
        let diagnostics = diagnostics_for(SIMPLE, &settings);
        assert_eq!(diagnostics.len(), 1, "everything is below 100%");
    }

    #[test]
    fn severity_escalates_below_the_error_threshold() {
        let mut settings = Settings::default();
        settings.health.quality_warn = 100.0;
        settings.health.quality_error = 100.0;
        let diagnostics = diagnostics_for(SIMPLE, &settings);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::Error));
    }

    #[test]
    fn disabling_diagnostics_clears_them() {
        let mut settings = Settings::default();
        settings.diagnostics.enabled = false;
        assert!(diagnostics_for(&tangled(), &settings).is_empty());
    }

    #[test]
    fn per_metric_advice_is_off_by_default_and_opt_in() {
        // A function long enough to fail one pillar while passing overall.
        let mut source = String::from("fn long() -> u32 {\n");
        for index in 0..60 {
            source.push_str(&format!("    let v{index} = {index};\n"));
        }
        source.push_str("    0\n}\n");

        let quiet = diagnostics_for(&source, &Settings::default());
        let mut settings = Settings::default();
        settings.diagnostics.per_metric = true;
        let loud = diagnostics_for(&source, &settings);
        assert!(loud.len() > quiet.len(), "quiet {quiet:?} loud {loud:?}");
        assert!(
            loud.iter()
                .all(|d| d.severity == Some(DiagnosticSeverity::Information))
        );
    }

    #[test]
    fn a_file_diagnostic_is_opt_in() {
        let mut settings = Settings::default();
        settings.diagnostics.file = true;
        settings.health.quality_error = 100.0;
        let diagnostics = diagnostics_for(&tangled(), &settings);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == Some(Code::String("file-quality".into()))),
            "{diagnostics:?}"
        );
    }
}
