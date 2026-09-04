use std::path::{Path, PathBuf};

use serde::Serialize;

use super::config::HealthConfig;
use crate::spaces::{FuncSpace, SpaceKind};

/// The qualitative band a quality score falls in, from poor to excellent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    /// 80% and above.
    #[default]
    Excellent,
    /// 50% and above.
    Good,
    /// 25% and above.
    Fair,
    /// Below 25%.
    Poor,
}

impl Grade {
    /// Returns the band a quality percentage falls in.
    pub fn of(quality: f64) -> Self {
        if quality >= 80.0 {
            Self::Excellent
        } else if quality >= 50.0 {
            Self::Good
        } else if quality >= 25.0 {
            Self::Fair
        } else {
            Self::Poor
        }
    }

    /// Returns the band as a lowercase word.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Fair => "fair",
            Self::Poor => "poor",
        }
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The raw value and 0-100 score of each pillar.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Scores {
    /// Cognitive complexity, and what it scores.
    pub complexity: f64,
    pub complexity_score: f64,
    /// Statements in the function, and what they score.
    pub length: f64,
    pub length_score: f64,
    /// Names held at the busiest statement, and what they score.
    pub working_memory: f64,
    pub working_memory_score: f64,
}

impl Scores {
    /// Returns the pillar with the lowest score, as `(name, score)`.
    ///
    /// Ties break toward complexity, then length: when two pillars are
    /// equally bad, the message names the one a reader feels first.
    pub fn worst_pillar(&self) -> (&'static str, f64) {
        let mut worst = ("complexity", self.complexity_score);
        if self.length_score < worst.1 {
            worst = ("method length", self.length_score);
        }
        if self.working_memory_score < worst.1 {
            worst = ("working memory", self.working_memory_score);
        }
        worst
    }
}

/// The health of one function.
#[derive(Clone, Debug, Serialize)]
pub struct FunctionHealth {
    /// The function's name, or `None` when it could not be parsed.
    pub name: Option<String>,
    /// First line of the function, 1-based.
    pub start_line: usize,
    /// Last line of the function, 1-based.
    pub end_line: usize,
    /// The blended quality percentage.
    pub quality: f64,
    /// The band `quality` falls in.
    pub grade: Grade,
    /// The pillars behind `quality`.
    pub scores: Scores,
}

impl FunctionHealth {
    /// Returns the function's name, or `<anonymous>` when it has none.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or("<anonymous>")
    }
}

/// The health of one file.
#[derive(Clone, Debug, Serialize)]
pub struct FileHealth {
    /// The analyzed path.
    pub path: PathBuf,
    /// Quality across the file's functions, weighted by their length.
    pub quality: f64,
    /// The band `quality` falls in.
    pub grade: Grade,
    /// The file's maintainability index, carried through from the metrics
    /// engine as a second opinion on the same file.
    pub maintainability_index: f64,
    /// Every function in the file, in source order.
    pub functions: Vec<FunctionHealth>,
}

impl FileHealth {
    /// Returns the lowest-scoring function of the file.
    pub fn worst(&self) -> Option<&FunctionHealth> {
        self.functions
            .iter()
            .min_by(|a, b| a.quality.total_cmp(&b.quality))
    }

    /// Returns the functions scoring below `threshold`, worst first.
    pub fn below(&self, threshold: f64) -> Vec<&FunctionHealth> {
        let mut below: Vec<&FunctionHealth> = self
            .functions
            .iter()
            .filter(|function| function.quality < threshold)
            .collect();
        below.sort_by(|a, b| a.quality.total_cmp(&b.quality));
        below
    }
}

/// The health of a set of files.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RepoHealth {
    /// Quality across every function analyzed, weighted by function length.
    pub quality: f64,
    /// The band `quality` falls in.
    pub grade: Grade,
    /// How many files were analyzed.
    pub files: usize,
    /// How many functions were analyzed.
    pub functions: usize,
    /// How many functions landed in each band.
    pub excellent: usize,
    pub good: usize,
    pub fair: usize,
    pub poor: usize,
    /// The worst functions found, worst first, capped by
    /// [`HealthConfig::worst_functions`].
    pub worst: Vec<(PathBuf, FunctionHealth)>,
}

/// Scores one raw metric value against its threshold.
///
/// `s(r) = 100 / (1 + (r/t)²)`: full marks at zero, 50% at the threshold,
/// decaying smoothly from there without ever reaching zero.
fn pillar_score(raw: f64, threshold: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return 100.0;
    }
    if !threshold.is_finite() || threshold <= 0.0 {
        // A zero threshold would mean "any amount at all is unacceptable";
        // treat it as unconfigured rather than dividing by zero.
        return 100.0;
    }
    let ratio = raw / threshold;
    100.0 / (1.0 + ratio * ratio)
}

/// Blends the three pillar scores into a quality percentage.
fn blend(scores: &Scores, config: &HealthConfig) -> f64 {
    let (wc, wl, ww) = config.weights.normalized();
    scores.complexity_score.powf(wc) * scores.length_score.powf(wl)
        // The geometric mean means one collapsed pillar pulls the whole
        // score down, which an arithmetic mean would let the others hide.
        * scores.working_memory_score.powf(ww)
}

/// Scores one function space.
fn function_health(space: &FuncSpace, config: &HealthConfig) -> FunctionHealth {
    let complexity = space.metrics.cognitive.cognitive();
    let length = space.metrics.loc.lloc();
    let working_memory = space.metrics.working_memory.wm();

    let scores = Scores {
        complexity,
        complexity_score: pillar_score(complexity, config.complexity_threshold),
        length,
        length_score: pillar_score(length, config.length_threshold),
        working_memory,
        working_memory_score: pillar_score(working_memory, config.working_memory_threshold),
    };
    let quality = blend(&scores, config);

    FunctionHealth {
        name: space.name.clone(),
        start_line: space.start_line,
        end_line: space.end_line,
        quality,
        grade: Grade::of(quality),
        scores,
    }
}

/// Collects every function space below `space`, in source order.
fn collect_functions(space: &FuncSpace, config: &HealthConfig, out: &mut Vec<FunctionHealth>) {
    for child in &space.spaces {
        if child.kind == SpaceKind::Function {
            out.push(function_health(child, config));
        }
        collect_functions(child, config, out);
    }
}

/// Averages function qualities, weighting each by its length.
///
/// Weighting by length keeps a file of one 300-statement disaster and five
/// one-line getters from reporting as healthy. With no length to weight by,
/// this falls back to a plain mean.
fn weighted_quality<'a>(functions: impl Iterator<Item = &'a FunctionHealth> + Clone) -> f64 {
    let total_weight: f64 = functions
        .clone()
        .map(|function| function.scores.length.max(0.0))
        .sum();
    if total_weight > 0.0 {
        functions
            .map(|function| function.quality * function.scores.length.max(0.0))
            .sum::<f64>()
            / total_weight
    } else {
        let mut count = 0usize;
        let total: f64 = functions
            .map(|function| {
                count += 1;
                function.quality
            })
            .sum();
        if count == 0 {
            100.0
        } else {
            total / count as f64
        }
    }
}

/// Scores a parsed file.
///
/// The `FuncSpace` is the one [`crate::get_function_spaces`] returns: the
/// unit space for the whole file, with every function nested below it.
pub fn file_health(space: &FuncSpace, path: &Path, config: &HealthConfig) -> FileHealth {
    let mut functions = Vec::new();
    collect_functions(space, config, &mut functions);
    functions.sort_by_key(|function| function.start_line);

    let quality = weighted_quality(functions.iter());

    FileHealth {
        path: path.to_path_buf(),
        quality,
        grade: Grade::of(quality),
        maintainability_index: space.metrics.mi.mi_visual_studio(),
        functions,
    }
}

impl RepoHealth {
    /// Aggregates file reports into one repository report.
    pub fn of<'a>(files: impl IntoIterator<Item = &'a FileHealth>, config: &HealthConfig) -> Self {
        let files: Vec<&FileHealth> = files.into_iter().collect();
        let all: Vec<(&PathBuf, &FunctionHealth)> = files
            .iter()
            .flat_map(|file| file.functions.iter().map(move |f| (&file.path, f)))
            .collect();

        let quality = weighted_quality(all.iter().map(|(_, function)| *function));

        let mut health = Self {
            quality,
            grade: Grade::of(quality),
            files: files.len(),
            functions: all.len(),
            ..Self::default()
        };
        for (_, function) in &all {
            match function.grade {
                Grade::Excellent => health.excellent += 1,
                Grade::Good => health.good += 1,
                Grade::Fair => health.fair += 1,
                Grade::Poor => health.poor += 1,
            }
        }

        let mut worst = all;
        worst.sort_by(|a, b| a.1.quality.total_cmp(&b.1.quality));
        health.worst = worst
            .into_iter()
            .take(config.worst_functions)
            .map(|(path, function)| (path.clone(), function.clone()))
            .collect();

        health
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LANG, get_function_spaces};

    fn health(lang: &LANG, source: &str, name: &str) -> FileHealth {
        let path = Path::new(name);
        let space = get_function_spaces(lang, source.as_bytes().to_vec(), path).unwrap();
        file_health(&space, path, &HealthConfig::default())
    }

    #[test]
    fn a_pillar_scores_full_marks_at_zero_and_half_at_its_threshold() {
        assert_eq!(pillar_score(0.0, 15.0), 100.0);
        assert_eq!(pillar_score(15.0, 15.0), 50.0);
        assert_eq!(pillar_score(30.0, 15.0), 20.0);
        assert_eq!(pillar_score(45.0, 15.0), 10.0);
    }

    #[test]
    fn a_pillar_score_decays_without_reaching_zero() {
        let huge = pillar_score(1_000_000.0, 15.0);
        assert!(huge > 0.0, "score should stay positive, got {huge}");
        assert!(huge < 0.001);
    }

    #[test]
    fn an_unset_threshold_does_not_divide_by_zero() {
        assert_eq!(pillar_score(42.0, 0.0), 100.0);
        assert_eq!(pillar_score(42.0, -1.0), 100.0);
    }

    #[test]
    fn equal_pillars_blend_to_their_common_value() {
        let scores = Scores {
            complexity: 0.0,
            complexity_score: 40.0,
            length: 0.0,
            length_score: 40.0,
            working_memory: 0.0,
            working_memory_score: 40.0,
        };
        let quality = blend(&scores, &HealthConfig::default());
        assert!((quality - 40.0).abs() < 1e-9, "got {quality}");
    }

    #[test]
    fn one_collapsed_pillar_drags_the_blend_below_the_arithmetic_mean() {
        let scores = Scores {
            complexity: 0.0,
            complexity_score: 1.0,
            length: 0.0,
            length_score: 100.0,
            working_memory: 0.0,
            working_memory_score: 100.0,
        };
        let quality = blend(&scores, &HealthConfig::default());
        assert!(
            quality < 67.0,
            "geometric mean should punish, got {quality}"
        );
        assert!(quality > 0.0);
    }

    #[test]
    fn weights_shift_the_blend_toward_the_weighted_pillar() {
        let scores = Scores {
            complexity: 0.0,
            complexity_score: 10.0,
            length: 0.0,
            length_score: 100.0,
            working_memory: 0.0,
            working_memory_score: 100.0,
        };
        let balanced = blend(&scores, &HealthConfig::default());
        let complexity_heavy = blend(
            &scores,
            &HealthConfig {
                weights: super::super::Weights {
                    complexity: 8.0,
                    length: 1.0,
                    working_memory: 1.0,
                },
                ..HealthConfig::default()
            },
        );
        assert!(complexity_heavy < balanced);
    }

    #[test]
    fn grades_sit_on_their_boundaries() {
        assert_eq!(Grade::of(100.0), Grade::Excellent);
        assert_eq!(Grade::of(80.0), Grade::Excellent);
        assert_eq!(Grade::of(79.9), Grade::Good);
        assert_eq!(Grade::of(50.0), Grade::Good);
        assert_eq!(Grade::of(49.9), Grade::Fair);
        assert_eq!(Grade::of(25.0), Grade::Fair);
        assert_eq!(Grade::of(24.9), Grade::Poor);
        assert_eq!(Grade::of(0.0), Grade::Poor);
    }

    #[test]
    fn a_simple_function_is_excellent() {
        let file = health(
            &LANG::Rust,
            "fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n",
            "a.rs",
        );
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].grade, Grade::Excellent);
    }

    #[test]
    fn a_deeply_nested_function_scores_worse_than_a_flat_one() {
        let flat = health(
            &LANG::Rust,
            "fn f(a: u32) -> u32 {
    if a > 1 { return 1; }
    if a > 2 { return 2; }
    if a > 3 { return 3; }
    0
}
",
            "flat.rs",
        );
        let nested = health(
            &LANG::Rust,
            "fn f(a: u32) -> u32 {
    if a > 1 {
        if a > 2 {
            if a > 3 {
                if a > 4 {
                    return 4;
                }
            }
        }
    }
    0
}
",
            "nested.rs",
        );
        assert!(
            nested.functions[0].quality < flat.functions[0].quality,
            "nested {} should score below flat {}",
            nested.functions[0].quality,
            flat.functions[0].quality
        );
    }

    #[test]
    fn a_file_without_functions_is_not_penalized() {
        let file = health(&LANG::Rust, "const A: u32 = 1;\n", "consts.rs");
        assert!(file.functions.is_empty());
        assert_eq!(file.quality, 100.0);
    }

    #[test]
    fn file_quality_is_weighted_by_function_length() {
        // One long bad function and one short good one: the file should
        // track the long one, not sit halfway between them.
        let mut long_bad = health(&LANG::Rust, "fn f() {}\n", "f.rs").functions;
        long_bad.clear();
        let functions = [
            FunctionHealth {
                name: Some("bad".into()),
                start_line: 1,
                end_line: 100,
                quality: 10.0,
                grade: Grade::Poor,
                scores: Scores {
                    complexity: 0.0,
                    complexity_score: 10.0,
                    length: 99.0,
                    length_score: 10.0,
                    working_memory: 0.0,
                    working_memory_score: 10.0,
                },
            },
            FunctionHealth {
                name: Some("good".into()),
                start_line: 101,
                end_line: 102,
                quality: 100.0,
                grade: Grade::Excellent,
                scores: Scores {
                    complexity: 0.0,
                    complexity_score: 100.0,
                    length: 1.0,
                    length_score: 100.0,
                    working_memory: 0.0,
                    working_memory_score: 100.0,
                },
            },
        ];
        let quality = weighted_quality(functions.iter());
        assert!(quality < 12.0, "got {quality}");
    }

    #[test]
    fn worst_reports_the_lowest_scoring_function() {
        let file = health(
            &LANG::Rust,
            "fn easy() -> u32 { 1 }

fn hard(a: u32) -> u32 {
    if a > 1 {
        if a > 2 {
            if a > 3 {
                return 3;
            }
        }
    }
    0
}
",
            "both.rs",
        );
        assert_eq!(file.worst().unwrap().display_name(), "hard");
    }

    #[test]
    fn below_lists_offenders_worst_first() {
        let file = health(
            &LANG::Rust,
            "fn easy() -> u32 { 1 }

fn hard(a: u32) -> u32 {
    if a > 1 {
        if a > 2 {
            if a > 3 {
                if a > 4 {
                    if a > 5 {
                        return 5;
                    }
                }
            }
        }
    }
    0
}
",
            "both.rs",
        );
        let offenders = file.below(90.0);
        assert_eq!(offenders.first().unwrap().display_name(), "hard");
    }

    #[test]
    fn repo_health_counts_bands_and_caps_the_worst_list() {
        let config = HealthConfig {
            worst_functions: 1,
            ..HealthConfig::default()
        };
        let a = health(&LANG::Rust, "fn a() -> u32 { 1 }\n", "a.rs");
        let b = health(
            &LANG::Rust,
            "fn b(x: u32) -> u32 {
    if x > 1 {
        if x > 2 {
            if x > 3 {
                if x > 4 {
                    return 4;
                }
            }
        }
    }
    0
}
",
            "b.rs",
        );
        let repo = RepoHealth::of([&a, &b], &config);
        assert_eq!(repo.files, 2);
        assert_eq!(repo.functions, 2);
        assert_eq!(repo.worst.len(), 1);
        assert_eq!(repo.worst[0].1.display_name(), "b");
        assert_eq!(
            repo.excellent + repo.good + repo.fair + repo.poor,
            repo.functions
        );
    }

    #[test]
    fn an_empty_repo_is_not_penalized() {
        let repo = RepoHealth::of([], &HealthConfig::default());
        assert_eq!(repo.functions, 0);
        assert_eq!(repo.quality, 100.0);
    }

    #[test]
    fn worst_pillar_names_the_lowest_score() {
        let scores = Scores {
            complexity: 0.0,
            complexity_score: 90.0,
            length: 0.0,
            length_score: 80.0,
            working_memory: 0.0,
            working_memory_score: 12.0,
        };
        assert_eq!(scores.worst_pillar().0, "working memory");
    }
}
