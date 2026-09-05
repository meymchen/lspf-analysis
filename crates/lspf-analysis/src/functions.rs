//! The per-function detail a client draws its own UI from.
//!
//! Hover renders one function as Markdown, and the file summary counts them
//! all without saying what any one of them measured. A client that wants to
//! draw its own list — a tree, a panel, a chart — needs the numbers rather
//! than a rendering of them, so it asks for them here.
//!
//! This is a request rather than another field on
//! [`FileHealthParams`](crate::status::FileHealthParams) because that
//! notification is pushed on every keystroke. The full breakdown of every
//! function is an order of magnitude more JSON, and only a client with the
//! view open has any use for it; it is pulled when something is listening.

use lspf::types::Uri;
use lspf::types::request::Request;
use lspf_analysis_core::health::{FileHealth, FunctionHealth, Pillar};
use serde::{Deserialize, Serialize};

/// `lspfAnalysis/functionHealth`: every function of one document, scored.
///
/// The result is `null` for a document the server has not analyzed — one it
/// cannot parse, or one no longer open — which is the same answer hover
/// gives for a position it has nothing to say about.
pub struct FunctionHealthRequest;

impl Request for FunctionHealthRequest {
    type Params = FunctionHealthParams;
    type Result = Option<FunctionHealthResult>;
    const METHOD: &'static str = "lspfAnalysis/functionHealth";
}

/// Which document to report on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionHealthParams {
    pub uri: Uri,
}

/// Every function of one document, in source order.
///
/// Source order rather than worst-first: the server does not know how the
/// client means to sort, and only the source order cannot be recovered from
/// the payload once it is lost.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionHealthResult {
    /// The document this describes, echoed so a client can drop an answer
    /// that arrived after it moved on to another file.
    pub uri: Uri,
    pub functions: Vec<FunctionDetail>,
}

/// One function, and every number behind its score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDetail {
    /// The function's name, or `<anonymous>` when it has none, matching what
    /// hover and the file summary call it.
    pub name: String,
    /// Where it starts and ends, 1-based, so a client can both navigate to it
    /// and tell whether the cursor is inside it.
    pub start_line: usize,
    pub end_line: usize,
    pub quality: f64,
    /// The band `quality` falls in, as a lowercase word.
    pub grade: String,
    /// The pillar that set `quality`, and the measure behind that pillar.
    /// Derivable from `pillars`, but every client would derive it the same
    /// way, and the tie-break belongs to the server that defined the order.
    pub weakest_pillar: String,
    pub weakest_metric: String,
    /// The four pillars, in the order the weights are given in.
    pub pillars: Vec<PillarDetail>,
}

/// One pillar of a function's score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PillarDetail {
    pub name: String,
    /// The worst of `measures`. Not the average: see
    /// [`lspf_analysis_core::health`].
    pub score: f64,
    pub measures: Vec<MeasureDetail>,
}

/// One metric, as measured and as scored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasureDetail {
    pub name: String,
    /// The raw value the engine computed.
    pub value: f64,
    /// The value that would score 50%, carried so a client can show what a
    /// number is judged against rather than only how it scored.
    pub threshold: f64,
    pub score: f64,
}

/// Describes every function of an analyzed file.
pub fn detail(uri: &Uri, report: &FileHealth) -> FunctionHealthResult {
    FunctionHealthResult {
        uri: uri.clone(),
        functions: report.functions.iter().map(describe).collect(),
    }
}

/// Describes one function down to its individual measures.
fn describe(function: &FunctionHealth) -> FunctionDetail {
    let weakest = function.scores.worst_pillar();
    FunctionDetail {
        name: function.display_name().to_string(),
        start_line: function.start_line,
        end_line: function.end_line,
        quality: function.quality,
        grade: function.grade.to_string(),
        weakest_pillar: weakest.name.to_string(),
        weakest_metric: weakest
            .worst_measure()
            .map(|measure| measure.name.to_string())
            .unwrap_or_default(),
        pillars: function.scores.pillars().map(pillar).to_vec(),
    }
}

fn pillar(pillar: &Pillar) -> PillarDetail {
    PillarDetail {
        name: pillar.name.to_string(),
        score: pillar.score,
        measures: pillar
            .measures
            .iter()
            .map(|measure| MeasureDetail {
                name: measure.name.to_string(),
                value: measure.value,
                threshold: measure.threshold,
                score: measure.score,
            })
            .collect(),
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

    fn result() -> FunctionHealthResult {
        let report = analyze(
            LANG::Rust,
            SOURCE.to_string(),
            Path::new("a.rs"),
            &HealthConfig::default(),
        )
        .unwrap();
        detail(&Uri::from_str("file:///a.rs").unwrap(), &report)
    }

    #[test]
    fn every_function_is_reported_in_source_order() {
        let result = result();
        assert_eq!(result.uri.as_str(), "file:///a.rs");
        let names: Vec<&str> = result
            .functions
            .iter()
            .map(|function| function.name.as_str())
            .collect();
        assert_eq!(names, ["add", "one"]);
        assert!(result.functions[0].start_line < result.functions[1].start_line);
    }

    #[test]
    fn a_function_carries_every_pillar_and_measure() {
        let result = result();
        let add = &result.functions[0];
        let pillars: Vec<&str> = add
            .pillars
            .iter()
            .map(|pillar| pillar.name.as_str())
            .collect();
        assert_eq!(
            pillars,
            ["control flow", "size", "vocabulary load", "interface"]
        );
        let measures: Vec<&str> = add
            .pillars
            .iter()
            .flat_map(|pillar| pillar.measures.iter())
            .map(|measure| measure.name.as_str())
            .collect();
        assert_eq!(
            measures,
            [
                "cognitive complexity",
                "cyclomatic complexity",
                "statements",
                "working memory",
                "Halstead difficulty",
                "parameters",
            ]
        );
    }

    #[test]
    fn a_measure_says_what_it_was_judged_against() {
        let result = result();
        let parameters = result.functions[0]
            .pillars
            .iter()
            .flat_map(|pillar| pillar.measures.iter())
            .find(|measure| measure.name == "parameters")
            .expect("the interface pillar has one measure");
        assert_eq!(parameters.value, 2.0, "`add` takes two");
        assert_eq!(parameters.threshold, 4.0, "the shipped default");
        assert!(parameters.score > 0.0 && parameters.score <= 100.0);
    }

    #[test]
    fn a_pillar_scores_at_its_worst_measure() {
        for function in result().functions {
            for pillar in &function.pillars {
                let worst = pillar
                    .measures
                    .iter()
                    .map(|measure| measure.score)
                    .fold(100.0_f64, f64::min);
                assert_eq!(pillar.score, worst, "{} of {}", pillar.name, function.name);
            }
        }
    }

    #[test]
    fn the_weakest_pillar_is_named_and_is_present_in_the_breakdown() {
        for function in result().functions {
            let weakest = function
                .pillars
                .iter()
                .find(|pillar| pillar.name == function.weakest_pillar)
                .unwrap_or_else(|| panic!("{} names a pillar it does not carry", function.name));
            let lowest = function
                .pillars
                .iter()
                .map(|pillar| pillar.score)
                .fold(f64::INFINITY, f64::min);
            assert_eq!(weakest.score, lowest, "{}", function.name);
            assert!(!function.weakest_metric.is_empty(), "{}", function.name);
        }
    }

    #[test]
    fn the_payload_survives_a_round_trip() {
        // The client decodes this, so what is serialized has to decode back
        // into the same thing, camelCase keys and all.
        let result = result();
        let json = serde_json::to_value(&result).unwrap();
        assert!(json["functions"][0]["startLine"].is_number(), "{json}");
        assert!(json["functions"][0]["weakestPillar"].is_string(), "{json}");
        let decoded: FunctionHealthResult = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, result);
    }

    #[test]
    fn the_method_name_is_namespaced_to_this_server() {
        assert_eq!(FunctionHealthRequest::METHOD, "lspfAnalysis/functionHealth");
    }
}
