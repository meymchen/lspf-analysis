//! Display text in the reader's language.
//!
//! Only text a person reads is translated. The vocabulary that travels in
//! [`status`](crate::status) and [`functions`](crate::functions) — pillar
//! names, metric names, grade words — stays English on the wire, because it
//! is what a client keys off, and is translated here only when it is being
//! rendered into a sentence. A payload that changed shape with the editor's
//! display language would not be a protocol.
//!
//! English is the source language: a string with no translation is rendered
//! as written rather than as a key, so an untranslated addition degrades to
//! English instead of to `hover.weakest.line`.
//!
//! Placeholders are `{0}`, `{1}`, … rather than named or positional-by-order,
//! so a translation can reorder them — which Chinese needs, since "41
//! cognitive complexity" reverses. It is also the convention `vscode.l10n`
//! uses, and the VS Code client translates the same vocabulary the same way.

/// The language display text is rendered in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    English,
    SimplifiedChinese,
}

impl Locale {
    /// Reads a locale from a client's language tag.
    ///
    /// Anything that is not Simplified Chinese is English: this server ships
    /// two languages, and a reader whose editor is in a third is better
    /// served by the language the numbers were named in than by a guess.
    pub fn of(tag: Option<&str>) -> Self {
        let Some(tag) = tag else {
            return Self::English;
        };
        // `zh_CN` and `zh-Hans-CN` both occur in the wild; VS Code sends
        // `zh-cn`.
        let tag = tag.to_ascii_lowercase().replace('_', "-");
        if !tag.starts_with("zh") {
            return Self::English;
        }
        // Traditional Chinese is a different translation, not this one.
        let traditional = ["hant", "tw", "hk", "mo"]
            .iter()
            .any(|region| tag.split('-').any(|part| part == *region));
        if traditional {
            Self::English
        } else {
            Self::SimplifiedChinese
        }
    }

    /// Translates one English source string.
    pub fn t(self, source: &'static str) -> &'static str {
        match self {
            Self::English => source,
            Self::SimplifiedChinese => zh_cn(source).unwrap_or(source),
        }
    }

    /// Translates a template and fills its `{0}`-style placeholders.
    pub fn fill(self, source: &'static str, args: &[&str]) -> String {
        fill(self.t(source), args)
    }
}

/// Substitutes `{0}`, `{1}`, … in a template.
///
/// A placeholder with no argument is left as written. A sentence with a
/// visible `{3}` in it is a bug that shows itself; one with a silent hole is
/// a bug that has to be hunted.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, after) = rest.split_at(open);
        out.push_str(before);
        let Some(close) = after.find('}') else {
            out.push_str(after);
            return out;
        };
        match after[1..close]
            .parse::<usize>()
            .ok()
            .and_then(|index| args.get(index))
        {
            Some(value) => out.push_str(value),
            None => out.push_str(&after[..=close]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The Simplified Chinese for one English source string, if it has one.
fn zh_cn(source: &str) -> Option<&'static str> {
    Some(match source {
        // The pillars, as `lspf_analysis_core::health` names them.
        "control flow" => "控制流",
        "size" => "规模",
        "vocabulary load" => "词汇负担",
        "interface" => "接口",
        "class design" => "类设计",

        // The metrics behind them.
        "cognitive complexity" => "认知复杂度",
        "cyclomatic complexity" => "圈复杂度",
        "statements" => "语句数",
        "working memory" => "工作记忆",
        "Halstead difficulty" => "Halstead 难度",
        "parameters" => "参数个数",
        "weighted methods" => "加权方法数",
        "public methods" => "公有方法数",
        "public attributes" => "公有属性数",

        // The bands a quality score falls in.
        "excellent" => "优秀",
        "good" => "良好",
        "fair" => "一般",
        "poor" => "较差",

        // The hover.
        "quality" => "质量",
        "pillar" => "支柱",
        "metric" => "指标",
        "value" => "数值",
        "grade" => "等级",
        "Weakest: **{0}** — {1} {2} against a threshold of {3}, scoring {4}% ({5})." => {
            "最弱：**{0}** —— {1} {2}，阈值 {3}，得分 {4}%（{5}）。"
        }

        // The diagnostics. `", "` is the separator between the measurements
        // listed at the end of a message, which is punctuation and so
        // belongs to the language it is read in.
        ", " => "，",
        "{0} ({1} {2})" => "{0}（{1} {2}）",
        "function `{0}`: quality {1}% ({2}), worst pillar {3} — {4}" => {
            "函数 `{0}`：质量 {1}%（{2}），最弱支柱 {3} —— {4}"
        }
        "function `{0}`: {1} {2}" => "函数 `{0}`：{2} {1}",
        "class `{0}`: quality {1}% ({2}) — {3}" => "类 `{0}`：质量 {1}%（{2}）—— {3}",
        "file quality {0}% ({1}) across {2} function" => "文件质量 {0}%（{1}），共 {2} 个函数",
        "file quality {0}% ({1}) across {2} functions" => "文件质量 {0}%（{1}），共 {2} 个函数",

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplified_chinese_is_recognized_however_it_is_spelled() {
        for tag in ["zh-cn", "zh-CN", "zh_CN", "zh", "zh-Hans", "zh-hans-cn"] {
            assert_eq!(
                Locale::of(Some(tag)),
                Locale::SimplifiedChinese,
                "{tag} is Simplified Chinese"
            );
        }
    }

    #[test]
    fn every_other_language_is_english() {
        for tag in ["en", "en-US", "ja", "de", "fr", "ko", ""] {
            assert_eq!(Locale::of(Some(tag)), Locale::English, "{tag}");
        }
        assert_eq!(Locale::of(None), Locale::English, "a client that sent none");
    }

    #[test]
    fn traditional_chinese_is_not_this_translation() {
        // A half-translated reading is worse than an English one.
        for tag in ["zh-tw", "zh-hk", "zh-Hant", "zh-Hant-TW"] {
            assert_eq!(Locale::of(Some(tag)), Locale::English, "{tag}");
        }
    }

    #[test]
    fn english_renders_its_source_unchanged() {
        assert_eq!(Locale::English.t("control flow"), "control flow");
        assert_eq!(
            Locale::English.fill("function `{0}`: {1} {2}", &["add", "41", "statements"]),
            "function `add`: 41 statements"
        );
    }

    #[test]
    fn an_untranslated_string_falls_back_to_english() {
        assert_eq!(
            Locale::SimplifiedChinese.t("a string nobody has translated"),
            "a string nobody has translated"
        );
    }

    #[test]
    fn a_translation_may_reorder_its_placeholders() {
        assert_eq!(
            Locale::SimplifiedChinese.fill("function `{0}`: {1} {2}", &["add", "41", "statements"]),
            "函数 `add`：statements 41",
            "the vocabulary is translated by its caller, the order by the template"
        );
    }

    #[test]
    fn a_placeholder_with_no_argument_is_left_visible() {
        assert_eq!(fill("{0} and {1}", &["one"]), "one and {1}");
        assert_eq!(fill("{}", &["one"]), "{}", "an index is required");
        assert_eq!(fill("{0", &["one"]), "{0", "an unclosed brace ends it");
    }

    #[test]
    fn text_without_placeholders_survives_intact() {
        assert_eq!(fill("nothing to fill", &["unused"]), "nothing to fill");
        assert_eq!(fill("", &[]), "");
    }
}
