//! Code health scoring.
//!
//! The metrics engine answers "how complex is this function?". This module
//! answers "is this function healthy?", by folding three metrics into a
//! single quality percentage, at three granularities: function, file, and
//! repository.
//!
//! # The three pillars
//!
//! Three metrics are computed for every function:
//!
//! | Pillar         | Metric                                          |
//! | -------------- | ----------------------------------------------- |
//! | Complexity     | cognitive complexity (Campbell)                 |
//! | Method length  | logical lines of code, i.e. statements           |
//! | Working memory | names held at the busiest statement             |
//!
//! # The formula
//!
//! Each pillar is scored on its own before the three are blended, so a
//! function is never rescued by being short if it is impenetrable.
//!
//! A raw value `r >= 0` with threshold `t > 0` scores
//!
//! ```text
//! s(r) = 100 / (1 + (r/t)²)
//! ```
//!
//! which gives `s(0) = 100`, `s(t) = 50`, `s(2t) = 20`, `s(3t) = 10`. The
//! curve is smooth and monotone and never leaves `(0, 100]`, so no clamping
//! is needed and no function is ever written off entirely.
//!
//! The three sub-scores blend into quality as a weighted geometric mean:
//!
//! ```text
//! quality = s_complexity^w₁ · s_length^w₂ · s_working_memory^w₃    (Σw = 1)
//! ```
//!
//! A geometric mean was chosen over an arithmetic one because one collapsed
//! pillar should drag the whole score down: a 200-statement function is hard
//! to work with no matter how flat its control flow is.

mod config;
mod score;

pub use config::{HealthConfig, Weights};
pub use score::{FileHealth, FunctionHealth, Grade, RepoHealth, Scores, file_health};
