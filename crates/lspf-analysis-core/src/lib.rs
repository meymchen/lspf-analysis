//! lspf-analysis-core computes code metrics from source code and scores the
//! result into a code health rating.
//!
//! The metrics engine is a fork of
//! <a href="https://github.com/mozilla/rust-code-analysis/" target="_blank">rust-code-analysis</a>,
//! which parses source code with
//! <a href="https://tree-sitter.github.io/tree-sitter/" target="_blank">Tree Sitter</a>.
//! On top of it this crate adds the [`health`] module, which turns raw metrics
//! into per-function, per-file, and per-repository quality scores.
//!
//! ## Supported Languages
//!
//! - Java
//! - JavaScript
//! - Python
//! - Rust
//! - Typescript
//! - Tsx
//!
//! ## Supported Metrics
//!
//! - CC: it calculates the code complexity examining the
//!   control flow of a program.
//! - SLOC: it counts the number of lines in a source file.
//! - PLOC: it counts the number of physical lines (instructions)
//!   contained in a source file.
//! - LLOC: it counts the number of logical lines (statements)
//!   contained in a source file.
//! - CLOC: it counts the number of comments in a source file.
//! - BLANK: it counts the number of blank lines in a source file.
//! - HALSTEAD: it is a suite that provides a series of information,
//!   such as the effort required to maintain the analyzed code,
//!   the size in bits to store the program, the difficulty to understand
//!   the code, an estimate of the number of bugs present in the codebase,
//!   and an estimate of the time needed to implement the software.
//! - MI: it is a suite that allows to evaluate the maintainability
//!   of a software.
//! - NOM: it counts the number of functions and closures
//!   in a file/trait/class.
//! - NEXITS: it counts the number of possible exit points
//!   from a method/function.
//! - NARGS: it counts the number of arguments of a function/method.
//! - WM: it counts the variables a reader has to keep in working memory
//!   at the busiest statement of a function.
//! - ABC: it counts the assignments, branches and conditions of a piece of
//!   code. Computed for `Java` only.
//! - WMC: it sums the cyclomatic complexities of the methods of a class.
//!   Computed for `Java` only.
//! - NPM: it counts the public methods of a class/interface. Computed for
//!   `Java` only.
//! - NPA: it counts the public attributes of a class/interface. Computed
//!   for `Java` only.
//!
//! ## Health Scoring
//!
//! [`health`] blends cognitive complexity, method length, and working memory
//! into a single quality percentage. See that module for the exact formula.

#![allow(clippy::upper_case_acronyms)]

mod getter;
mod macros;

mod node;
pub use crate::node::*;

mod metrics;
pub use metrics::*;

mod languages;
pub(crate) use languages::*;

mod checker;

mod output;
pub use output::*;

mod spaces;
pub use crate::spaces::*;

pub mod health;

mod langs;
pub use crate::langs::*;

mod tools;
pub use crate::tools::*;

mod concurrent_files;
pub use crate::concurrent_files::*;

mod traits;
pub use crate::traits::*;

mod parser;
pub use crate::parser::*;
