//! The file-level summary, sent to the client rather than drawn in the text.
//!
//! A file's quality belongs to the whole document, so it has no honest range
//! to sit on; a diagnostic on line 1 is a guess. Instead the server pushes
//! this summary after every analysis and lets the client decide where to put
//! it — the VS Code extension shows it in the status bar.

use lspf::types::Uri;
use lspf::types::notification::Notification;
use lspf_analysis_core::health::FileHealth;
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
    /// The lowest-scoring function, when the file has one.
    pub worst: Option<WorstFunction>,
}

/// The one function a reader should open first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstFunction {
    pub name: String,
    pub quality: f64,
    pub grade: String,
    /// Where it starts, 1-based, so the client can offer to go there.
    pub line: usize,
}

/// Summarizes an analyzed file for the client.
pub fn summarize(uri: &Uri, report: &FileHealth, settings: &Settings) -> FileHealthParams {
    FileHealthParams {
        uri: uri.clone(),
        quality: report.quality,
        grade: report.grade.to_string(),
        functions: report.functions.len(),
        below: report.below(settings.health.quality_warn).len(),
        worst: report.worst().map(|function| WorstFunction {
            name: function.display_name().to_string(),
            quality: function.quality,
            grade: function.grade.to_string(),
            line: function.start_line,
        }),
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
    fn the_worst_function_is_named() {
        let worst = summary(&Settings::default()).worst.unwrap();
        assert!(["add", "one"].contains(&worst.name.as_str()), "{worst:?}");
        assert_eq!(worst.grade, "excellent");
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
