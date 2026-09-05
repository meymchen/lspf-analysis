//! The file-level summary, sent to the client rather than drawn in the text.
//!
//! A file's quality belongs to the whole document, so it has no honest range
//! to sit on; a diagnostic on line 1 is a guess. Instead the server pushes
//! this summary after every analysis and lets the client decide where to put
//! it — the VS Code extension shows it in the status bar.

use lspf::types::Uri;
use lspf::types::notification::Notification;
use lspf_analysis_core::health::{FileHealth, FunctionHealth, Grade};
use serde::{Deserialize, Serialize};

use crate::config::Settings;

/// `lspfAnalysis/fileHealth`: one summary per analyzed document.
///
/// A client that does not know the method ignores it, which is what the
/// specification requires of an unknown `$`-less notification anyway, so
/// sending it unconditionally costs nothing.
pub struct FileHealthNotification;

impl Notification for FileHealthNotification {
    type Params = FileHealthParams;
    const METHOD: &'static str = "lspfAnalysis/fileHealth";
}

/// What the client is told about a file it has open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHealthParams {
    /// The document this describes.
    pub uri: Uri,
    /// Quality across the file's functions, weighted by their length.
    pub quality: f64,
    /// The band `quality` falls in, as a lowercase word.
    pub grade: String,
    /// How many functions the file has.
    pub functions: usize,
    /// How many of them are below the warning threshold, and so already
    /// carry a diagnostic.
    pub below: usize,
    /// How many functions landed in each band, so a client can show the
    /// shape of the file rather than one number standing for all of it.
    pub bands: Bands,
    /// The functions a reader should open first, worst first, capped by
    /// [`HealthConfig::worst_functions`](lspf_analysis_core::health::HealthConfig).
    pub worst: Vec<WorstFunction>,
}

/// How a file's functions are spread across the four bands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bands {
    pub excellent: usize,
    pub good: usize,
    pub fair: usize,
    pub poor: usize,
}

/// One function worth opening, and enough to navigate to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstFunction {
    pub name: String,
    pub quality: f64,
    pub grade: String,
    /// Where it starts, 1-based, so the client can offer to go there.
    pub line: usize,
    /// The pillar that set its score, and the metric behind that pillar.
    pub weakest_pillar: String,
    pub weakest_metric: String,
}

/// Summarizes an analyzed file for the client.
pub fn summarize(uri: &Uri, report: &FileHealth, settings: &Settings) -> FileHealthParams {
    let mut bands = Bands::default();
    for function in &report.functions {
        let counter = match function.grade {
            Grade::Excellent => &mut bands.excellent,
            Grade::Good => &mut bands.good,
            Grade::Fair => &mut bands.fair,
            Grade::Poor => &mut bands.poor,
        };
        *counter += 1;
    }

    let mut ranked: Vec<&FunctionHealth> = report.functions.iter().collect();
    ranked.sort_by(|a, b| a.quality.total_cmp(&b.quality));

    FileHealthParams {
        uri: uri.clone(),
        quality: report.quality,
        grade: report.grade.to_string(),
        functions: report.functions.len(),
        below: report.below(settings.health.quality_warn).len(),
        bands,
        worst: ranked
            .into_iter()
            .take(settings.health.worst_functions)
            .map(describe)
            .collect(),
    }
}

/// Describes one function for the client's list.
fn describe(function: &FunctionHealth) -> WorstFunction {
    let pillar = function.scores.worst_pillar();
    WorstFunction {
        name: function.display_name().to_string(),
        quality: function.quality,
        grade: function.grade.to_string(),
        line: function.start_line,
        weakest_pillar: pillar.name.to_string(),
        weakest_metric: pillar
            .worst_measure()
            .map(|metric| metric.name.to_string())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::analyze;
    use lspf_analysis_core::LANG;
    use lspf_analysis_core::health::HealthConfig;
    use std::path::Path;
    use std::str::FromStr;

    const SOURCE: &str =
        "fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n\nfn one() -> u32 {\n    1\n}\n";

    fn summary(settings: &Settings) -> FileHealthParams {
        let report = analyze(
            LANG::Rust,
            SOURCE.to_string(),
            Path::new("a.rs"),
            &HealthConfig::default(),
        )
        .unwrap();
        summarize(&Uri::from_str("file:///a.rs").unwrap(), &report, settings)
    }

    #[test]
    fn a_summary_counts_every_function() {
        let summary = summary(&Settings::default());
        assert_eq!(summary.functions, 2);
        assert_eq!(summary.below, 0);
        assert_eq!(summary.grade, "excellent");
        assert_eq!(summary.uri.as_str(), "file:///a.rs");
    }

    #[test]
    fn the_worst_functions_are_listed_worst_first() {
        let summary = summary(&Settings::default());
        assert_eq!(summary.worst.len(), 2, "both functions fit under the cap");
        assert!(
            summary.worst[0].quality <= summary.worst[1].quality,
            "{:?}",
            summary.worst
        );
        let worst = &summary.worst[0];
        assert!(["add", "one"].contains(&worst.name.as_str()), "{worst:?}");
        assert_eq!(worst.grade, "excellent");
        assert!(!worst.weakest_pillar.is_empty());
        assert!(!worst.weakest_metric.is_empty());
    }

    #[test]
    fn the_bands_account_for_every_function() {
        let bands = summary(&Settings::default()).bands;
        assert_eq!(bands.excellent, 2);
        assert_eq!(
            bands.excellent + bands.good + bands.fair + bands.poor,
            2,
            "{bands:?}"
        );
    }

    #[test]
    fn the_worst_list_is_capped_by_the_configured_count() {
        let mut settings = Settings::default();
        settings.health.worst_functions = 1;
        assert_eq!(summary(&settings).worst.len(), 1);
    }

    #[test]
    fn the_count_below_follows_the_warning_threshold() {
        let mut settings = Settings::default();
        settings.health.quality_warn = 100.0;
        assert_eq!(summary(&settings).below, 2);
    }

    #[test]
    fn the_method_name_is_namespaced_to_this_server() {
        assert_eq!(FileHealthNotification::METHOD, "lspfAnalysis/fileHealth");
    }
}
