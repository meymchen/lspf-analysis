use serde::{Deserialize, Serialize};

/// How many pillars a *function's* quality score blends.
///
/// A class is scored on one pillar of its own and does not go through this
/// blend; see [`HealthConfig::wmc_threshold`] and the
/// [module docs](crate::health).
pub const PILLARS: usize = 4;

/// Where each measure's score crosses 50%, and where a quality score stops
/// being acceptable.
///
/// Every field has a default, and deserialization fills in the ones the
/// caller left out, so an editor can send `{"complexityThreshold": 20}` and
/// get the defaults for everything else. The keys of the three original
/// thresholds are unchanged, so a configuration written for the earlier
/// three-pillar model still means what it meant.
///
/// Where the defaults come from is documented per field; the reasoning and
/// its sources are in the [module docs](crate::health).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthConfig {
    /// Cognitive complexity scoring 50%. Above roughly twice this, a
    /// function's control flow no longer fits in one reading.
    pub complexity_threshold: f64,
    /// Cyclomatic complexity scoring 50%. McCabe's own recommended upper
    /// bound for a module, which he set at 10.
    pub cyclomatic_threshold: f64,
    /// Statements (logical lines) scoring 50%. The middle of the three
    /// unit-size boundaries the SIG maintainability model uses.
    pub length_threshold: f64,
    /// Working memory scoring 50%. Estimates of human working-memory
    /// capacity run from about 4 to about 9 items; the default sits at the
    /// top of that range so only genuinely wide statements are penalized.
    pub working_memory_threshold: f64,
    /// Halstead difficulty scoring 50%.
    pub halstead_difficulty_threshold: f64,
    /// Parameter count scoring 50%. The SIG model treats 2 as the low-risk
    /// bound for a unit's interface; 4 is where the "long parameter list"
    /// smell is conventionally called.
    pub parameters_threshold: f64,
    /// Weighted methods per class scoring 50%. Where a benchmark of Java
    /// systems puts the boundary between a common class and an uncommon one.
    pub wmc_threshold: f64,
    /// Public methods per class scoring 50%. The same benchmark's boundary
    /// for methods per class; a class's public methods are a subset of them,
    /// so this is a conservative bound.
    pub public_methods_threshold: f64,
    /// Public attributes per class scoring 50%. The same benchmark's
    /// boundary for fields per class, read the same conservative way.
    pub public_attributes_threshold: f64,
    /// How the pillars are weighted against each other.
    pub weights: Weights,
    /// Quality below this is reported as a warning.
    pub quality_warn: f64,
    /// Quality below this is reported as an error rather than a warning.
    pub quality_error: f64,
    /// How many of the worst functions a repository report lists.
    pub worst_functions: usize,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            complexity_threshold: 15.0,
            cyclomatic_threshold: 10.0,
            length_threshold: 30.0,
            working_memory_threshold: 8.0,
            halstead_difficulty_threshold: 12.0,
            parameters_threshold: 4.0,
            wmc_threshold: 34.0,
            public_methods_threshold: 14.0,
            public_attributes_threshold: 8.0,
            weights: Weights::default(),
            quality_warn: 25.0,
            quality_error: 10.0,
            worst_functions: 5,
        }
    }
}

/// The relative weight of each pillar in the quality blend.
///
/// The weights are normalized by their sum before use, so `{1, 1, 1, 1}` and
/// `{2, 2, 2, 2}` mean the same thing and no caller can produce a quality
/// score outside `(0, 100]` by mis-setting them.
///
/// `interface` starts at half the others: a wide signature is a real signal
/// but a narrower one than the three that describe a function's body, and
/// giving it a full share would let parameter count alone decide a verdict.
///
/// The first four weight the pillars of a *function's* score. `class_design`
/// is not one of them: it weighs a file's classes against its functions, and
/// only in files where classes were actually scored.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Weights {
    /// Control flow: cognitive and cyclomatic complexity.
    pub complexity: f64,
    /// Size: logical lines of code.
    pub length: f64,
    /// Vocabulary load: working memory and Halstead difficulty.
    pub working_memory: f64,
    /// Interface: parameter count.
    pub interface: f64,
    /// How much a file's class scores count against its function scores.
    ///
    /// Half a share, like `interface`: a class's shape matters, but the
    /// functions inside it are what a reader actually works through.
    pub class_design: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            complexity: 1.0,
            length: 1.0,
            working_memory: 1.0,
            interface: 0.5,
            class_design: 0.5,
        }
    }
}

impl Weights {
    /// Returns the weights scaled to sum to 1, in pillar order.
    ///
    /// Negative or non-finite weights, and a total of zero, fall back to
    /// equal shares rather than producing a nonsense score.
    pub(crate) fn normalized(&self) -> [f64; PILLARS] {
        let equal = [1.0 / PILLARS as f64; PILLARS];
        let values = [
            self.complexity,
            self.length,
            self.working_memory,
            self.interface,
        ];
        if values.iter().any(|w| !w.is_finite() || *w < 0.0) {
            return equal;
        }
        let total: f64 = values.iter().sum();
        if total <= 0.0 {
            return equal;
        }
        values.map(|value| value / total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_give_the_interface_half_a_share() {
        let weights = Weights::default().normalized();
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!((weights[0] - 1.0 / 3.5).abs() < 1e-12);
        assert!((weights[3] - 0.5 / 3.5).abs() < 1e-12);
    }

    #[test]
    fn weights_are_scaled_to_sum_to_one() {
        let weights = Weights {
            complexity: 2.0,
            length: 1.0,
            working_memory: 0.5,
            interface: 0.5,
            ..Weights::default()
        }
        .normalized();
        assert!((weights[0] - 0.5).abs() < 1e-12);
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn nonsense_weights_fall_back_to_equal_shares() {
        for weights in [
            Weights {
                complexity: 0.0,
                length: 0.0,
                working_memory: 0.0,
                interface: 0.0,
                ..Weights::default()
            },
            Weights {
                complexity: -1.0,
                length: 1.0,
                working_memory: 1.0,
                interface: 1.0,
                ..Weights::default()
            },
            Weights {
                complexity: f64::NAN,
                length: 1.0,
                working_memory: 1.0,
                interface: 1.0,
                ..Weights::default()
            },
        ] {
            let weights = weights.normalized();
            assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
            assert!((weights[0] - 0.25).abs() < 1e-12);
        }
    }

    #[test]
    fn partial_settings_keep_the_other_defaults() {
        let config: HealthConfig = serde_json::from_str(r#"{"complexityThreshold": 20}"#).unwrap();
        assert_eq!(config.complexity_threshold, 20.0);
        assert_eq!(config.length_threshold, 30.0);
        assert_eq!(config.cyclomatic_threshold, 10.0);
        assert_eq!(config.quality_warn, 25.0);
    }

    #[test]
    fn a_three_pillar_configuration_still_loads() {
        // What an editor configured for the earlier model sends.
        let config: HealthConfig = serde_json::from_str(
            r#"{"complexityThreshold": 20, "lengthThreshold": 40,
                "workingMemoryThreshold": 6,
                "weights": {"complexity": 2, "length": 1, "workingMemory": 1}}"#,
        )
        .unwrap();
        assert_eq!(config.complexity_threshold, 20.0);
        assert_eq!(config.weights.complexity, 2.0);
        assert_eq!(config.weights.interface, 0.5, "the new pillar's default");
    }
}
