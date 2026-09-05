use std::path::{Path, PathBuf};

use serde::Serialize;

use super::config::{HealthConfig, PILLARS};
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

/// One metric, as measured and as scored.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Measure {
    /// What the metric is called in a report.
    pub name: &'static str,
    /// The raw value the engine computed.
    pub value: f64,
    /// The value that would score 50%. Carried so a report can say what a
    /// number is being judged against, rather than only how it scored.
    pub threshold: f64,
    /// What that value scores, from 0 to 100.
    pub score: f64,
}

/// One pillar of the quality score, and the metrics behind it.
///
/// A pillar takes the **worst** of its measures rather than their average.
/// Two metrics of the same property are partly redundant — cognitive and
/// cyclomatic complexity both describe control flow — so averaging them
/// would let a function hide a bad number behind a good one, while adding
/// them as separate pillars would count the same property twice.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pillar {
    /// What the pillar is called in a report.
    pub name: &'static str,
    /// The worst of `measures`, from 0 to 100.
    pub score: f64,
    /// What was measured, in the order the pillar defines them.
    pub measures: Vec<Measure>,
}

impl Pillar {
    /// Builds a pillar from its measures, scoring it at the worst of them.
    ///
    /// A pillar with no measures scores 100: nothing was found to hold
    /// against the function, which is different from finding a problem.
    fn of(name: &'static str, measures: Vec<Measure>) -> Self {
        let score = measures
            .iter()
            .map(|measure| measure.score)
            .fold(100.0_f64, f64::min);
        Self {
            name,
            score,
            measures,
        }
    }

    /// Returns the measure that set this pillar's score.
    pub fn worst_measure(&self) -> Option<&Measure> {
        self.measures
            .iter()
            .min_by(|a, b| a.score.total_cmp(&b.score))
    }
}

/// Every pillar behind one function's quality score.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scores {
    /// How tangled the control flow is.
    pub control_flow: Pillar,
    /// How much the function does.
    pub size: Pillar,
    /// How many names have to be held at once.
    pub vocabulary: Pillar,
    /// How wide the signature is.
    pub interface: Pillar,
}

impl Scores {
    /// Returns the pillars in the order the weights are given in.
    pub fn pillars(&self) -> [&Pillar; PILLARS] {
        [
            &self.control_flow,
            &self.size,
            &self.vocabulary,
            &self.interface,
        ]
    }

    /// Returns the function's size in statements.
    ///
    /// This is the one measure read outside its own pillar: a file's
    /// quality weights each function by how much of the file it is.
    pub fn statements(&self) -> f64 {
        self.size.measures.first().map_or(0.0, |m| m.value)
    }

    /// Returns the lowest-scoring pillar.
    ///
    /// Ties break toward the earlier pillar: when two are equally bad, the
    /// message names the one a reader feels first.
    pub fn worst_pillar(&self) -> &Pillar {
        self.pillars()
            .into_iter()
            .min_by(|a, b| a.score.total_cmp(&b.score))
            .expect("there is always at least one pillar")
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

/// The one pillar behind a class's quality score.
///
/// A class is not a function and is not scored like one: none of the four
/// function pillars is defined for it, and the three measures that are — the
/// complexity it carries, and how much of itself it exposes — are all
/// properties of its design.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassScores {
    /// How heavy the class is, and how much of it is public.
    pub class_design: Pillar,
}

/// The health of one class or interface.
///
/// Only produced for languages whose engine actually computes the
/// class-level metrics. A `class` space in a language that does not compute
/// them would score a constant 100 and say nothing.
#[derive(Clone, Debug, Serialize)]
pub struct ClassHealth {
    /// The class's name, or `None` when it could not be parsed.
    pub name: Option<String>,
    /// Whether this is a class or an interface.
    pub kind: SpaceKind,
    /// First line of the class, 1-based.
    pub start_line: usize,
    /// Last line of the class, 1-based.
    pub end_line: usize,
    /// The pillar's score, which is the class's quality.
    pub quality: f64,
    /// The band `quality` falls in.
    pub grade: Grade,
    /// How many methods the class defines, used to weight it against the
    /// other classes of the file.
    pub methods: f64,
    /// The pillar behind `quality`.
    pub scores: ClassScores,
}

impl ClassHealth {
    /// Returns the class's name, or `<anonymous>` when it has none.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or("<anonymous>")
    }
}

/// The health of one file.
#[derive(Clone, Debug, Serialize)]
pub struct FileHealth {
    /// The analyzed path.
    pub path: PathBuf,
    /// Quality across the file's functions, weighted by their length, and —
    /// where the language computes class metrics — its classes.
    pub quality: f64,
    /// The band `quality` falls in.
    pub grade: Grade,
    /// The file's maintainability index, carried through from the metrics
    /// engine as a second opinion on the same file.
    pub maintainability_index: f64,
    /// Every function in the file, in source order.
    pub functions: Vec<FunctionHealth>,
    /// Every scored class in the file, in source order. Empty for languages
    /// whose engine does not compute the class-level metrics.
    pub classes: Vec<ClassHealth>,
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
    /// How many classes were scored. Zero unless the sources include a
    /// language whose engine computes the class-level metrics.
    pub classes: usize,
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

/// Blends the pillar scores into a quality percentage.
///
/// The geometric mean means one collapsed pillar pulls the whole score
/// down, which an arithmetic mean would let the others hide.
fn blend(scores: &Scores, config: &HealthConfig) -> f64 {
    let weights = config.weights.normalized();
    scores
        .pillars()
        .iter()
        .zip(weights)
        .map(|(pillar, weight)| pillar.score.powf(weight))
        .product()
}

/// Replaces a non-finite measurement with zero.
///
/// Halstead difficulty divides by the number of distinct operands, so a
/// function that uses none — an empty body, a bare `pass` — yields NaN.
/// Nothing was measured there, which scores as nothing held against it.
fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// Measures `value` against `threshold` under `name`.
fn measure(name: &'static str, value: f64, threshold: f64) -> Measure {
    Measure {
        name,
        value,
        threshold,
        score: pillar_score(value, threshold),
    }
}

/// Scores one function space.
///
/// Which metrics feed which pillar, and why the rest of what the engine
/// computes is deliberately left out, is set out in the
/// [module docs](crate::health).
fn function_health(space: &FuncSpace, config: &HealthConfig) -> FunctionHealth {
    let metrics = &space.metrics;
    // A closure's parameters are counted separately from a named
    // function's; a space is one or the other, so the larger is its own.
    let parameters = metrics.nargs.fn_args().max(metrics.nargs.closure_args());

    let scores = Scores {
        control_flow: Pillar::of(
            "control flow",
            vec![
                measure(
                    "cognitive complexity",
                    metrics.cognitive.cognitive(),
                    config.complexity_threshold,
                ),
                measure(
                    "cyclomatic complexity",
                    metrics.cyclomatic.cyclomatic(),
                    config.cyclomatic_threshold,
                ),
            ],
        ),
        size: Pillar::of(
            "size",
            vec![measure(
                "statements",
                metrics.loc.lloc(),
                config.length_threshold,
            )],
        ),
        vocabulary: Pillar::of(
            "vocabulary load",
            vec![
                measure(
                    "working memory",
                    metrics.working_memory.wm(),
                    config.working_memory_threshold,
                ),
                measure(
                    "Halstead difficulty",
                    finite(metrics.halstead.difficulty()),
                    config.halstead_difficulty_threshold,
                ),
            ],
        ),
        interface: Pillar::of(
            "interface",
            vec![measure(
                "parameters",
                parameters,
                config.parameters_threshold,
            )],
        ),
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

/// Scores one class or interface space.
///
/// A class's measures come in two flavours — class and interface — because
/// an interface's members are implicitly public and the engine counts them
/// separately. Which set applies is the space's own kind.
fn class_health(space: &FuncSpace, config: &HealthConfig) -> ClassHealth {
    let metrics = &space.metrics;
    let interface = space.kind == SpaceKind::Interface;
    let (weighted_methods, public_methods, public_attributes, methods) = if interface {
        (
            metrics.wmc.interface_wmc(),
            metrics.npm.interface_npm(),
            metrics.npa.interface_npa(),
            metrics.npm.interface_nm(),
        )
    } else {
        (
            metrics.wmc.class_wmc(),
            metrics.npm.class_npm(),
            metrics.npa.class_npa(),
            metrics.npm.class_nm(),
        )
    };

    let scores = ClassScores {
        class_design: Pillar::of(
            "class design",
            vec![
                measure(
                    "weighted methods",
                    finite(weighted_methods),
                    config.wmc_threshold,
                ),
                measure(
                    "public methods",
                    finite(public_methods),
                    config.public_methods_threshold,
                ),
                measure(
                    "public attributes",
                    finite(public_attributes),
                    config.public_attributes_threshold,
                ),
            ],
        ),
    };
    let quality = scores.class_design.score;

    ClassHealth {
        name: space.name.clone(),
        kind: space.kind,
        start_line: space.start_line,
        end_line: space.end_line,
        quality,
        grade: Grade::of(quality),
        methods: finite(methods),
        scores,
    }
}

/// Collects every scored class space below `space`, in source order.
///
/// A class space only counts when its language really computed the
/// class-level metrics. `Wmc::compute` is a no-op for every grammar but
/// Java, and a no-op leaves the space kind at `Unknown`, which is exactly
/// what `wmc::Stats::is_disabled` reports. So a Python or JavaScript class
/// is skipped rather than scored a meaningless 100.
fn collect_classes(space: &FuncSpace, config: &HealthConfig, out: &mut Vec<ClassHealth>) {
    for child in &space.spaces {
        if matches!(child.kind, SpaceKind::Class | SpaceKind::Interface)
            && !child.metrics.wmc.is_disabled()
        {
            out.push(class_health(child, config));
        }
        collect_classes(child, config, out);
    }
}

/// Averages class qualities, weighting each by how many methods it defines.
///
/// A one-method holder should not weigh as much as a forty-method service,
/// for the same reason a file weights its functions by length.
fn weighted_class_quality(classes: &[ClassHealth]) -> f64 {
    let total_weight: f64 = classes.iter().map(|class| class.methods.max(0.0)).sum();
    if total_weight > 0.0 {
        classes
            .iter()
            .map(|class| class.quality * class.methods.max(0.0))
            .sum::<f64>()
            / total_weight
    } else if classes.is_empty() {
        100.0
    } else {
        classes.iter().map(|class| class.quality).sum::<f64>() / classes.len() as f64
    }
}

/// Blends a file's function quality with its class quality.
///
/// With no scored classes this is the function quality untouched, so a file
/// in a language without class-level metrics scores exactly what it scored
/// before classes were scored at all.
fn blend_with_classes(functions: f64, classes: &[ClassHealth], config: &HealthConfig) -> f64 {
    if classes.is_empty() {
        return functions;
    }
    let weight = config.weights.class_design;
    // A negative or non-finite weight is a misconfiguration, not a request
    // to invert the score; fall back to counting only the functions.
    if !weight.is_finite() || weight <= 0.0 {
        return functions;
    }
    let share = weight / (1.0 + weight);
    functions.powf(1.0 - share) * weighted_class_quality(classes).powf(share)
}

/// Averages function qualities, weighting each by its length.
///
/// Weighting by length keeps a file of one 300-statement disaster and five
/// one-line getters from reporting as healthy. With no length to weight by,
/// this falls back to a plain mean.
fn weighted_quality<'a>(functions: impl Iterator<Item = &'a FunctionHealth> + Clone) -> f64 {
    let total_weight: f64 = functions
        .clone()
        .map(|function| function.scores.statements().max(0.0))
        .sum();
    if total_weight > 0.0 {
        functions
            .map(|function| function.quality * function.scores.statements().max(0.0))
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

    let mut classes = Vec::new();
    collect_classes(space, config, &mut classes);
    classes.sort_by_key(|class| class.start_line);

    let quality = blend_with_classes(weighted_quality(functions.iter()), &classes, config);

    FileHealth {
        path: path.to_path_buf(),
        quality,
        grade: Grade::of(quality),
        maintainability_index: space.metrics.mi.mi_visual_studio(),
        functions,
        classes,
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

        let classes: Vec<ClassHealth> = files
            .iter()
            .flat_map(|file| file.classes.iter().cloned())
            .collect();

        let quality = blend_with_classes(
            weighted_quality(all.iter().map(|(_, function)| *function)),
            &classes,
            config,
        );

        let mut health = Self {
            quality,
            grade: Grade::of(quality),
            files: files.len(),
            functions: all.len(),
            classes: classes.len(),
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

    /// Builds scores with the four pillars set to the given values.
    fn scored(control_flow: f64, size: f64, vocabulary: f64, interface: f64) -> Scores {
        let pillar = |name, score| Pillar {
            name,
            score,
            measures: Vec::new(),
        };
        Scores {
            control_flow: pillar("control flow", control_flow),
            size: pillar("size", size),
            vocabulary: pillar("vocabulary load", vocabulary),
            interface: pillar("interface", interface),
        }
    }

    /// Scores with every pillar at `score` and a size of `statements`.
    fn sized(score: f64, statements: f64) -> Scores {
        let mut scores = scored(score, score, score, score);
        scores.size.measures = vec![measure("statements", statements, 30.0)];
        scores
    }

    #[test]
    fn equal_pillars_blend_to_their_common_value() {
        let quality = blend(&scored(40.0, 40.0, 40.0, 40.0), &HealthConfig::default());
        assert!((quality - 40.0).abs() < 1e-9, "got {quality}");
    }

    #[test]
    fn one_collapsed_pillar_drags_the_blend_below_the_arithmetic_mean() {
        let quality = blend(&scored(1.0, 100.0, 100.0, 100.0), &HealthConfig::default());
        assert!(
            quality < 75.0,
            "geometric mean should punish, got {quality}"
        );
        assert!(quality > 0.0);
    }

    #[test]
    fn weights_shift_the_blend_toward_the_weighted_pillar() {
        let scores = scored(10.0, 100.0, 100.0, 100.0);
        let balanced = blend(&scores, &HealthConfig::default());
        let complexity_heavy = blend(
            &scores,
            &HealthConfig {
                weights: super::super::Weights {
                    complexity: 8.0,
                    length: 1.0,
                    working_memory: 1.0,
                    interface: 0.5,
                    ..super::super::Weights::default()
                },
                ..HealthConfig::default()
            },
        );
        assert!(complexity_heavy < balanced);
    }

    #[test]
    fn the_interface_pillar_counts_for_half_of_another() {
        let config = HealthConfig::default();
        let bad_interface = blend(&scored(100.0, 100.0, 100.0, 10.0), &config);
        let bad_size = blend(&scored(100.0, 10.0, 100.0, 100.0), &config);
        assert!(
            bad_interface > bad_size,
            "a wide signature should cost less than a long body: {bad_interface} vs {bad_size}"
        );
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

    /// A Java class with one public method and one public field, plus an
    /// interface, so both class kinds are exercised.
    const JAVA_CLASSES: &str = "interface Shape {
    double area();
}

public class Box implements Shape {
    public int side;
    private int hidden;

    public double area() {
        return side * side;
    }

    private void reset() {
        side = 0;
    }
}
";

    #[test]
    fn a_java_class_and_interface_are_scored() {
        let file = health(&LANG::Java, JAVA_CLASSES, "Box.java");
        let names: Vec<&str> = file
            .classes
            .iter()
            .map(|class| class.display_name())
            .collect();
        assert_eq!(names, ["Shape", "Box"], "in source order");
        assert_eq!(file.classes[0].kind, SpaceKind::Interface);
        assert_eq!(file.classes[1].kind, SpaceKind::Class);

        let measures = &file.classes[1].scores.class_design.measures;
        let named = |name: &str| {
            measures
                .iter()
                .find(|measure| measure.name == name)
                .unwrap_or_else(|| panic!("no {name} measure"))
                .value
        };
        // `area` is public, `reset` is not; `side` is public, `hidden` is not.
        assert_eq!(named("public methods"), 1.0);
        assert_eq!(named("public attributes"), 1.0);
        // `area` and `reset` are one apiece.
        assert_eq!(named("weighted methods"), 2.0);
    }

    #[test]
    fn a_class_in_a_language_without_class_metrics_is_not_scored() {
        // Python, JavaScript and TypeScript all produce class spaces, but
        // none of them computes the class-level metrics, so scoring them
        // would report a meaningless 100.
        for (lang, source, name) in [
            (
                LANG::Python,
                "class C:\n    def m(self):\n        return 1\n",
                "c.py",
            ),
            (
                LANG::Javascript,
                "class C {\n    m() { return 1; }\n}\n",
                "c.js",
            ),
            (
                LANG::Typescript,
                "class C {\n    m(): number { return 1; }\n}\n",
                "c.ts",
            ),
        ] {
            let file = health(&lang, source, name);
            assert!(
                file.classes.is_empty(),
                "{name} should have no scored classes, got {:?}",
                file.classes.len()
            );
        }
    }

    #[test]
    fn classes_only_move_a_file_score_where_they_are_scored() {
        // Without scored classes the blend is the function quality itself,
        // so every language but Java scores exactly what it scored before
        // classes were scored at all.
        let file = health(
            &LANG::Python,
            "class C:\n    def m(self):\n        return 1\n",
            "c.py",
        );
        assert_eq!(file.quality, weighted_quality(file.functions.iter()));

        // With them, the file's own classes pull on the score.
        let java = health(&LANG::Java, JAVA_CLASSES, "Box.java");
        let functions_only = weighted_quality(java.functions.iter());
        assert!(
            (java.quality - functions_only).abs() > f64::EPSILON,
            "the classes should move the file score: {} vs {functions_only}",
            java.quality
        );
    }

    #[test]
    fn a_wide_class_scores_below_a_narrow_one() {
        let narrow = health(&LANG::Java, JAVA_CLASSES, "Box.java");
        let mut wide = String::from("public class Wide {\n");
        for index in 0..40 {
            wide.push_str(&format!("    public int field{index};\n"));
            wide.push_str(&format!("    public int get{index}() {{ return 1; }}\n"));
        }
        wide.push_str("}\n");
        let wide = health(&LANG::Java, &wide, "Wide.java");
        assert!(
            wide.classes[0].quality < narrow.classes[1].quality,
            "a 40-method class {} should score below a 2-method one {}",
            wide.classes[0].quality,
            narrow.classes[1].quality
        );
        assert_eq!(wide.classes[0].grade, Grade::Poor);
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
                scores: sized(10.0, 99.0),
            },
            FunctionHealth {
                name: Some("good".into()),
                start_line: 101,
                end_line: 102,
                quality: 100.0,
                grade: Grade::Excellent,
                scores: sized(100.0, 1.0),
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
        let scores = scored(90.0, 80.0, 12.0, 100.0);
        assert_eq!(scores.worst_pillar().name, "vocabulary load");
        assert_eq!(
            scored(90.0, 80.0, 70.0, 12.0).worst_pillar().name,
            "interface"
        );
    }

    #[test]
    fn a_pillar_scores_at_its_worst_measure_and_names_it() {
        let pillar = Pillar::of(
            "control flow",
            vec![
                measure("cognitive complexity", 1.0, 15.0),
                measure("cyclomatic complexity", 40.0, 10.0),
            ],
        );
        assert!(pillar.score < 10.0, "got {}", pillar.score);
        assert_eq!(
            pillar.worst_measure().unwrap().name,
            "cyclomatic complexity"
        );
    }

    #[test]
    fn a_pillar_with_nothing_to_measure_is_not_held_against_a_function() {
        assert_eq!(Pillar::of("interface", Vec::new()).score, 100.0);
    }

    #[test]
    fn a_non_finite_measurement_counts_as_nothing_measured() {
        // Halstead difficulty over a body with no operands.
        assert_eq!(finite(f64::NAN), 0.0);
        assert_eq!(finite(f64::INFINITY), 0.0);
        assert_eq!(
            measure("Halstead difficulty", finite(f64::NAN), 12.0).score,
            100.0
        );
    }

    #[test]
    fn cyclomatic_complexity_can_set_the_control_flow_score_alone() {
        // A flat `match` with many arms: cognitive complexity stays low
        // while cyclomatic complexity climbs.
        let mut source = String::from("fn dispatch(value: u32) -> u32 {\n    match value {\n");
        for index in 0..30 {
            source.push_str(&format!("        {index} => {index},\n"));
        }
        source.push_str("        _ => 0,\n    }\n}\n");
        let report = health(&LANG::Rust, &source, "a.rs");
        let function = &report.functions[0];
        let worst = function.scores.control_flow.worst_measure().unwrap();
        assert_eq!(worst.name, "cyclomatic complexity", "{:?}", function.scores);
    }

    #[test]
    fn a_wide_signature_is_reported_on_the_interface_pillar() {
        let report = health(
            &LANG::Rust,
            "fn wide(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32) -> u32 {\n    a\n}\n",
            "a.rs",
        );
        let function = &report.functions[0];
        assert_eq!(function.scores.interface.measures[0].value, 6.0);
        assert!(
            function.scores.interface.score < 50.0,
            "six parameters should score below the four-parameter threshold, got {}",
            function.scores.interface.score
        );
    }
}
