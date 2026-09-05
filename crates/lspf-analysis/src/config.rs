//! Server settings, and how they are read off the wire.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use lspf_analysis_core::health::HealthConfig;

use crate::i18n::Locale;

/// The settings section this server reads, in `initializationOptions` and in
/// `workspace/didChangeConfiguration`.
pub const SECTION: &str = "lspfAnalysis";

/// Everything an editor can configure.
///
/// Missing fields take their defaults, so `{"lspfAnalysis": {"diagnostics":
/// {"perMetric": true}}}` is a complete, valid configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Thresholds and weights for the health score.
    pub health: HealthConfig,
    /// Which diagnostics get published.
    pub diagnostics: DiagnosticsSettings,
    /// The language to render hovers and diagnostic messages in, as an
    /// editor's language tag — `zh-cn`, `en`. Unset, or anything this server
    /// has no translation for, renders English. Only display text follows it;
    /// what travels in `lspfAnalysis/fileHealth` and
    /// `lspfAnalysis/functionHealth` stays English either way.
    pub locale: Option<String>,
}

/// Which of the available diagnostics the server publishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiagnosticsSettings {
    /// Publish anything at all. Turning this off clears what is displayed.
    pub enabled: bool,
    /// Also report a function whose quality passes but which has one badly
    /// scoring pillar. Off by default: it is advice, not a problem.
    pub per_metric: bool,
    /// Also report the file as a whole when its quality is bad enough to be
    /// an error. Off by default, since the offending functions are already
    /// reported individually.
    pub file: bool,
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            per_metric: false,
            file: false,
        }
    }
}

impl Settings {
    /// The locale display text is rendered in.
    pub fn locale(&self) -> Locale {
        Locale::of(self.locale.as_deref())
    }

    /// Reads settings out of a client-supplied value.
    ///
    /// The value is what the client sent for `initializationOptions` or
    /// `workspace/didChangeConfiguration`: either the section itself or an
    /// object containing it under [`SECTION`]. Anything unparseable yields
    /// `None`, leaving the previous settings in place — a typo in a config
    /// file should not silently reset a project's thresholds.
    pub fn from_value(value: &Value) -> Option<Self> {
        let section = value.get(SECTION).unwrap_or(value);
        if !section.is_object() {
            return None;
        }
        match serde_json::from_value(section.clone()) {
            Ok(settings) => Some(settings),
            Err(error) => {
                tracing::warn!(%error, "ignoring unreadable {SECTION} settings");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_publish_only_function_diagnostics() {
        let settings = Settings::default();
        assert!(settings.diagnostics.enabled);
        assert!(!settings.diagnostics.per_metric);
        assert!(!settings.diagnostics.file);
        assert_eq!(settings.health.quality_warn, 25.0);
    }

    #[test]
    fn settings_are_read_from_the_named_section() {
        let value = json!({ "lspfAnalysis": { "health": { "qualityWarn": 60.0 } } });
        let settings = Settings::from_value(&value).unwrap();
        assert_eq!(settings.health.quality_warn, 60.0);
    }

    #[test]
    fn settings_are_read_from_a_bare_section() {
        let value = json!({ "health": { "qualityWarn": 60.0 } });
        let settings = Settings::from_value(&value).unwrap();
        assert_eq!(settings.health.quality_warn, 60.0);
    }

    #[test]
    fn omitted_fields_keep_their_defaults() {
        let value = json!({ "lspfAnalysis": { "diagnostics": { "perMetric": true } } });
        let settings = Settings::from_value(&value).unwrap();
        assert!(settings.diagnostics.per_metric);
        assert!(settings.diagnostics.enabled);
        assert_eq!(settings.health.complexity_threshold, 15.0);
    }

    #[test]
    fn an_unreadable_section_is_ignored_rather_than_reset() {
        let value = json!({ "lspfAnalysis": { "health": { "qualityWarn": "not a number" } } });
        assert_eq!(Settings::from_value(&value), None);
    }

    #[test]
    fn the_locale_is_read_from_the_client_and_defaults_to_english() {
        assert_eq!(Settings::default().locale(), Locale::English);
        let value = json!({ "lspfAnalysis": { "locale": "zh-cn" } });
        let settings = Settings::from_value(&value).unwrap();
        assert_eq!(settings.locale(), Locale::SimplifiedChinese);
    }

    #[test]
    fn a_non_object_is_ignored() {
        assert_eq!(Settings::from_value(&json!("nonsense")), None);
        assert_eq!(Settings::from_value(&json!(null)), None);
    }
}
