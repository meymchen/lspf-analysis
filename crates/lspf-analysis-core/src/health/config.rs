use serde::{Deserialize, Serialize};

/// Where each pillar's score crosses 50%, and where a quality score stops
/// being acceptable.
///
/// Every field has a default, and deserialization fills in the ones the
/// caller left out, so an editor can send `{"complexityThreshold": 20}` and
/// get the defaults for everything else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthConfig {
    /// Cognitive complexity scoring 50%. Above roughly twice this, a
    /// function's control flow no longer fits in one reading.
    pub complexity_threshold: f64,
    /// Statements (logical lines) scoring 50%.
    pub length_threshold: f64,
    /// Working memory scoring 50%. Human working memory holds 5 to 9 items,
    /// so the default sits at the top of that range.
    pub working_memory_threshold: f64,
    /// How the three pillars are weighted against each other.
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
            length_threshold: 30.0,
            working_memory_threshold: 8.0,
            weights: Weights::default(),
            quality_warn: 25.0,
            quality_error: 10.0,
            worst_functions: 5,
        }
    }
}

/// The relative weight of each pillar in the quality blend.
///
/// The weights are normalized by their sum before use, so `{1, 1, 1}` and
/// `{2, 2, 2}` mean the same thing and no caller can produce a quality score
/// outside `(0, 100]` by mis-setting them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Weights {
    pub complexity: f64,
    pub length: f64,
    pub working_memory: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            complexity: 1.0,
            length: 1.0,
            working_memory: 1.0,
        }
    }
}

impl Weights {
    /// Returns the weights scaled to sum to 1.
    ///
    /// Negative or non-finite weights, and a total of zero, fall back to
    /// equal thirds rather than producing a nonsense score.
    pub(crate) fn normalized(&self) -> (f64, f64, f64) {
        let values = [self.complexity, self.length, self.working_memory];
        if values.iter().any(|w| !w.is_finite() || *w < 0.0) {
            return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
        }
        let total: f64 = values.iter().sum();
        if total <= 0.0 {
            return (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0);
        }
        (values[0] / total, values[1] / total, values[2] / total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_equal_thirds() {
        let (a, b, c) = Weights::default().normalized();
        assert!((a - 1.0 / 3.0).abs() < 1e-12);
        assert!((b - 1.0 / 3.0).abs() < 1e-12);
        assert!((c - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn weights_are_scaled_to_sum_to_one() {
        let weights = Weights {
            complexity: 2.0,
            length: 1.0,
            working_memory: 1.0,
        };
        let (a, b, c) = weights.normalized();
        assert!((a - 0.5).abs() < 1e-12);
        assert!((a + b + c - 1.0).abs() < 1e-12);
    }

    #[test]
    fn nonsense_weights_fall_back_to_equal_thirds() {
        for weights in [
            Weights {
                complexity: 0.0,
                length: 0.0,
                working_memory: 0.0,
            },
            Weights {
                complexity: -1.0,
                length: 1.0,
                working_memory: 1.0,
            },
            Weights {
                complexity: f64::NAN,
                length: 1.0,
                working_memory: 1.0,
            },
        ] {
            let (a, b, c) = weights.normalized();
            assert!((a + b + c - 1.0).abs() < 1e-12);
            assert!((a - 1.0 / 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn partial_settings_keep_the_other_defaults() {
        let config: HealthConfig = serde_json::from_str(r#"{"complexityThreshold": 20}"#).unwrap();
        assert_eq!(config.complexity_threshold, 20.0);
        assert_eq!(config.length_threshold, 30.0);
        assert_eq!(config.quality_warn, 25.0);
    }
}
