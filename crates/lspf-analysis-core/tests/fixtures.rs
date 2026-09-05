//! Snapshots of the metrics and the health scores for one small file per
//! supported language.
//!
//! These are the regression net for grammar upgrades and for scoring
//! changes: if a tree-sitter bump or a threshold change moves a number, the
//! diff shows exactly which metric moved and by how much.

use std::path::{Path, PathBuf};

use lspf_analysis_core::health::{FileHealth, HealthConfig, RepoHealth, file_health};
use lspf_analysis_core::{FuncSpace, get_function_spaces, guess_language, read_file_with_eol};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Every fixture, one per supported grammar plus the deliberately tangled
/// `nested.rs`.
const NAMES: [&str; 7] = [
    "simple.rs",
    "nested.rs",
    "simple.py",
    "simple.js",
    "simple.ts",
    "simple.tsx",
    "simple.java",
];

/// Everything a fixture asserts: the metrics tree, and the health derived
/// from it.
#[derive(serde::Serialize)]
struct Snapshot {
    metrics: FuncSpace,
    health: FileHealth,
}

fn analyze(name: &str) -> Snapshot {
    let path = Path::new(FIXTURES).join(name);
    let source = read_file_with_eol(&path)
        .expect("the fixture is readable")
        .expect("the fixture is not empty");
    let language = guess_language(&source, &path)
        .0
        .unwrap_or_else(|| panic!("no language for {name}"));
    let metrics = get_function_spaces(&language, source, &path)
        .unwrap_or_else(|| panic!("no spaces in {name}"));
    let health = file_health(&metrics, &path, &HealthConfig::default());
    Snapshot { metrics, health }
}

fn assert_fixture(name: &str) {
    insta::assert_yaml_snapshot!(name, analyze(name), {
        // Floating point values differ in the last places between systems.
        ".metrics.metrics.*.*" => insta::rounded_redaction(3),
        ".metrics.spaces[].**.metrics.*.*" => insta::rounded_redaction(3),
        ".health.quality" => insta::rounded_redaction(3),
        ".health.maintainability_index" => insta::rounded_redaction(3),
        ".health.functions[].quality" => insta::rounded_redaction(3),
        ".health.functions[].scores.*.score" => insta::rounded_redaction(3),
        ".health.functions[].scores.*.measures[].value" => insta::rounded_redaction(3),
        ".health.functions[].scores.*.measures[].score" => insta::rounded_redaction(3),
        ".health.classes[].quality" => insta::rounded_redaction(3),
        ".health.classes[].scores.*.score" => insta::rounded_redaction(3),
        ".health.classes[].scores.*.measures[].value" => insta::rounded_redaction(3),
        ".health.classes[].scores.*.measures[].score" => insta::rounded_redaction(3),
        // Paths differ between checkouts.
        ".metrics.name" => "[filepath]",
        ".health.path" => "[filepath]",
    });
}

#[test]
fn rust_simple() {
    assert_fixture("simple.rs");
}

#[test]
fn rust_nested() {
    assert_fixture("nested.rs");
}

#[test]
fn python_simple() {
    assert_fixture("simple.py");
}

#[test]
fn javascript_simple() {
    assert_fixture("simple.js");
}

#[test]
fn typescript_simple() {
    assert_fixture("simple.ts");
}

#[test]
fn tsx_simple() {
    assert_fixture("simple.tsx");
}

#[test]
fn java_simple() {
    assert_fixture("simple.java");
}

#[test]
fn every_supported_language_is_covered() {
    let mut languages: Vec<&'static str> = NAMES
        .iter()
        .map(|name| {
            let path = PathBuf::from(FIXTURES).join(name);
            let source = read_file_with_eol(&path).unwrap().unwrap();
            guess_language(&source, &path).0.unwrap().get_name()
        })
        .collect();
    languages.sort_unstable();
    languages.dedup();
    // Tsx and Typescript share a display name, so five distinct names cover
    // all six parsers.
    assert_eq!(
        languages,
        ["java", "javascript", "python", "rust", "typescript"]
    );
}

#[test]
fn the_nested_fixture_scores_worse_than_the_simple_one() {
    let simple = analyze("simple.rs").health;
    let nested = analyze("nested.rs").health;
    assert!(
        nested.quality < simple.quality,
        "nested {} should score below simple {}",
        nested.quality,
        simple.quality
    );
}

#[test]
fn a_repository_report_spans_every_fixture() {
    let files: Vec<FileHealth> = NAMES.iter().map(|name| analyze(name).health).collect();

    let repo = RepoHealth::of(files.iter(), &HealthConfig::default());
    assert_eq!(repo.files, 7);
    assert_eq!(
        repo.functions, 14,
        "two functions per fixture, one in nested.rs, plus the Java interface's method"
    );
    assert_eq!(
        repo.classes, 2,
        "only Java has class-level metrics: the interface and the class"
    );
    assert_eq!(
        repo.excellent + repo.good + repo.fair + repo.poor,
        repo.functions
    );
    assert_eq!(
        repo.worst
            .first()
            .map(|(_, f)| f.display_name().to_string()),
        Some("classify".to_string()),
        "the nested fixture's function is the worst in the set"
    );
}
