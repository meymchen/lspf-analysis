//! Turning a health report into LSP diagnostics.

use lspf::PositionEncoding;
use lspf::types::{Code, Diagnostic, DiagnosticSeverity};
use lspf_analysis_core::health::{FileHealth, FunctionHealth, HealthConfig};

use crate::config::Settings;
use crate::document::line_range;
use crate::i18n::Locale;

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
    let locale = settings.locale();
    let mut diagnostics = Vec::new();

    for function in &report.functions {
        if function.quality < config.quality_warn {
            diagnostics.push(quality_diagnostic(function, text, encoding, config, locale));
        } else if settings.diagnostics.per_metric {
            diagnostics.extend(pillar_diagnostics(function, text, encoding, locale));
        }
    }

    if settings.diagnostics.file && report.quality < config.quality_error {
        let count = report.functions.len();
        diagnostics.push(Diagnostic {
            range: line_range(text, 1, encoding),
            severity: Some(DiagnosticSeverity::Warning),
            code: Some(Code::String("file-quality".into())),
            source: Some(SOURCE.into()),
            // Singular and plural are separate source strings rather than a
            // suffix, because a language without plurals cannot be given one
            // by appending to a translation.
            message: locale
                .fill(
                    if count == 1 {
                        "file quality {0}% ({1}) across {2} function"
                    } else {
                        "file quality {0}% ({1}) across {2} functions"
                    },
                    &[
                        &format!("{:.0}", report.quality),
                        locale.t(report.grade.as_str()),
                        &count.to_string(),
                    ],
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
    locale: Locale,
) -> Diagnostic {
    let severity = if function.quality < config.quality_error {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };
    let pillar = function.scores.worst_pillar();
    // Naming the measurement that set the pillar's score turns "this is
    // complex" into something the reader can act on.
    let cause = pillar.worst_measure().map_or_else(
        || locale.t(pillar.name).to_string(),
        |metric| {
            locale.fill(
                "{0} ({1} {2})",
                &[
                    locale.t(pillar.name),
                    locale.t(metric.name),
                    &format!("{:.0}", metric.value),
                ],
            )
        },
    );
    Diagnostic {
        range: line_range(text, function.start_line, encoding),
        severity: Some(severity),
        code: Some(Code::String("quality".into())),
        source: Some(SOURCE.into()),
        message: locale
            .fill(
                "function `{0}`: quality {1}% ({2}), worst pillar {3} — {4}",
                &[
                    function.display_name(),
                    &format!("{:.0}", function.quality),
                    locale.t(function.grade.as_str()),
                    &cause,
                    &summarize(function, locale),
                ],
            )
            .into(),
        ..Diagnostic::default()
    }
}

/// Lists every measurement behind a function's score, worst pillar first.
fn summarize(function: &FunctionHealth, locale: Locale) -> String {
    function
        .scores
        .pillars()
        .iter()
        .flat_map(|pillar| pillar.measures.iter())
        .map(|metric| format!("{} {:.0}", locale.t(metric.name), metric.value))
        .collect::<Vec<_>>()
        // The separator is translated too: a list joined with a half-width
        // comma reads wrong in the middle of a Chinese sentence.
        .join(locale.t(", "))
}

/// Advisory diagnostics for a function that passes overall but has one
/// pillar sitting well past its threshold.
fn pillar_diagnostics(
    function: &FunctionHealth,
    text: &str,
    encoding: PositionEncoding,
    locale: Locale,
) -> Vec<Diagnostic> {
    function
        .scores
        .pillars()
        .iter()
        .flat_map(|pillar| pillar.measures.iter())
        .filter(|metric| metric.score < PILLAR_FLOOR)
        .map(|metric| Diagnostic {
            range: line_range(text, function.start_line, encoding),
            severity: Some(DiagnosticSeverity::Information),
            code: Some(Code::String(metric_code(metric.name).into())),
            source: Some(SOURCE.into()),
            message: locale
                .fill(
                    "function `{0}`: {1} {2}",
                    &[
                        function.display_name(),
                        &format!("{:.0}", metric.value),
                        locale.t(metric.name),
                    ],
                )
                .into(),
            ..Diagnostic::default()
        })
        .collect()
}

/// The stable `code` a per-metric diagnostic carries.
///
/// Editors and suppression comments key off this, so it is spelled out here
/// rather than derived from the display name, which is free to change.
fn metric_code(metric: &str) -> &'static str {
    match metric {
        "cognitive complexity" => "cognitive-complexity",
        "cyclomatic complexity" => "cyclomatic-complexity",
        "statements" => "method-length",
        "working memory" => "working-memory",
        "Halstead difficulty" => "halstead-difficulty",
        "parameters" => "parameters",
        _ => "metric",
    }
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
    fn a_chinese_reader_gets_chinese_messages() {
        let settings = Settings {
            locale: Some("zh-cn".into()),
            ..Settings::default()
        };
        let diagnostics = diagnostics_for(&tangled(), &settings);
        let text = message(&diagnostics[0]);
        assert!(text.starts_with("函数 `tangled`："), "{text}");
        assert!(text.contains("最弱支柱 控制流"), "{text}");
        assert!(text.contains("认知复杂度"), "{text}");
        assert!(text.contains("较差"), "{text}");
        assert!(!text.contains("quality"), "{text}");
    }

    #[test]
    fn the_code_a_diagnostic_carries_is_the_same_in_every_language() {
        // Editors and suppression comments key off it.
        let mut settings = Settings::default();
        settings.diagnostics.per_metric = true;
        settings.health.quality_warn = 0.0;
        let english = diagnostics_for(&tangled(), &settings);
        settings.locale = Some("zh-cn".into());
        let chinese = diagnostics_for(&tangled(), &settings);

        let codes = |diagnostics: &[Diagnostic]| -> Vec<Option<Code>> {
            diagnostics.iter().map(|d| d.code.clone()).collect()
        };
        assert!(!english.is_empty());
        assert_eq!(codes(&english), codes(&chinese));
        assert_ne!(
            message(&english[0]),
            message(&chinese[0]),
            "only the message follows the language"
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
