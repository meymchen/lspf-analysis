//! Server settings, and how they are read off the wire.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use lspf_analysis_core::health::HealthConfig;

use crate::i18n::Locale;
use crate::theme::{Rgb, Theme, ThemeKind};

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
    /// The appearance the reader's editor is drawn in, so that a grade letter
    /// can be coloured to read against it. Unset — which is every client that
    /// speaks plain LSP — renders the one palette that does its best at both
    /// ends. See [`crate::theme`].
    pub theme: Option<ThemeSettings>,
    /// What the client's Markdown renderer keeps, for a client whose LSP stack
    /// will not let it say so in `general.markdown.allowedTags`. See
    /// [`MarkdownSettings`].
    pub markdown: Option<MarkdownSettings>,
}

/// A second way to declare what a client's Markdown renderer keeps.
///
/// `general.markdown.allowedTags` in `initialize` is the standard place, and
/// it is where VS Code and IntelliJ say it. Visual Studio cannot: its
/// `ILanguageClient` exposes no hook for shaping client capabilities, and the
/// platform sends `initialize` without consulting the middle layer that would
/// otherwise be the way in. What it does control completely is
/// `initializationOptions`, so the same declaration is accepted here.
///
/// The two sources are unioned rather than ranked. They are the same claim
/// made in two places, so there is no case where they disagree and a
/// precedence rule would be needed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkdownSettings {
    /// The HTML tags the client's Markdown renderer keeps, spelled as LSP
    /// spells them, because it is the same list.
    pub allowed_tags: Option<Vec<String>>,
}

/// What a client reports about its appearance.
///
/// Both fields are loose strings and neither is validated here. A colour this
/// server cannot read has to degrade to no colour, and nothing more: these
/// travel in the same section as the health thresholds, and
/// [`Settings::from_value`] drops the whole section when any part of it fails
/// to parse. A typo in a theme colour must not be able to reset a project's
/// thresholds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeSettings {
    /// `light`, `dark`, or `highContrast`. This is the reader's intent, which
    /// no colour value implies, so only the client can say it.
    pub kind: Option<String>,
    /// The hover surface's real colour, as `#RRGGBB`, when the client can
    /// read one. This is a fact about the editor, used only to fit the
    /// palette's colours to it.
    pub background: Option<String>,
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

    /// The tags the client declared here rather than in its capabilities.
    pub fn declared_tags(&self) -> Option<&[String]> {
        self.markdown.as_ref()?.allowed_tags.as_deref()
    }

    /// The appearance to fit hover colours to, if the client reported one.
    ///
    /// `None` means the client said nothing, which is a different case from
    /// having said something unreadable: the former gets the palette that
    /// serves both ends, the latter gets its kind's stand-in surface. A
    /// section present but empty counts as having said nothing.
    pub fn theme(&self) -> Option<Theme> {
        let theme = self.theme.as_ref()?;
        if theme.kind.is_none() && theme.background.is_none() {
            return None;
        }
        Some(Theme {
            kind: theme.kind.as_deref().map_or_else(
                // A client that reported a surface but no kind told us the
                // fact without the intent; its own luminance names the kind.
                || {
                    theme
                        .background
                        .as_deref()
                        .and_then(Rgb::parse)
                        .map_or(ThemeKind::default(), ThemeKind::of_surface)
                },
                ThemeKind::of,
            ),
            background: theme.background.as_deref().and_then(Rgb::parse),
        })
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
    fn a_client_that_reports_no_theme_leaves_it_unset() {
        assert_eq!(Settings::default().theme(), None);
        // A section that is present but says nothing is the same as absent:
        // it names neither the intent nor the surface.
        let value = json!({ "lspfAnalysis": { "theme": {} } });
        assert_eq!(Settings::from_value(&value).unwrap().theme(), None);
    }

    #[test]
    fn a_theme_is_read_as_an_intent_and_a_surface() {
        let value = json!({
            "lspfAnalysis": { "theme": { "kind": "light", "background": "#f5f5f5" } }
        });
        let theme = Settings::from_value(&value).unwrap().theme().unwrap();
        assert_eq!(theme.kind, ThemeKind::Light);
        assert_eq!(theme.background, Rgb::parse("#f5f5f5"));
    }

    #[test]
    fn a_surface_without_an_intent_names_its_own_kind() {
        // A client can read a colour but not a kind. Light and dark it can
        // work out; high contrast it could never have implied.
        let with = |background: &str| {
            let value = json!({ "lspfAnalysis": { "theme": { "background": background } } });
            Settings::from_value(&value).unwrap().theme().unwrap().kind
        };
        assert_eq!(with("#1f1f1f"), ThemeKind::Dark);
        assert_eq!(with("#ffffff"), ThemeKind::Light);
    }

    #[test]
    fn an_unreadable_colour_costs_the_colour_and_nothing_else() {
        // The trap this guards: the theme rides in the same section as the
        // thresholds, and `from_value` drops the whole section when any part
        // of it fails to parse. A colour has to degrade on its own.
        let value = json!({
            "lspfAnalysis": {
                "health": { "qualityWarn": 60.0 },
                "locale": "zh-cn",
                "theme": { "kind": "dark", "background": "rgb(31, 31, 31)" }
            }
        });
        let settings = Settings::from_value(&value).expect("the section still parses");
        assert_eq!(settings.health.quality_warn, 60.0, "the threshold survives");
        assert_eq!(settings.locale(), Locale::SimplifiedChinese);
        let theme = settings.theme().unwrap();
        assert_eq!(theme.kind, ThemeKind::Dark, "the intent survives");
        assert_eq!(theme.background, None, "only the colour is lost");
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
