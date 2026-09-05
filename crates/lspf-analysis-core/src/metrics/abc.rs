use serde::Serialize;
use serde::ser::{SerializeStruct, Serializer};
use std::fmt;

use crate::checker::Checker;
use crate::macros::implement_metric_trait;
use crate::node::Node;
use crate::*;

/// The `ABC` metric.
///
/// The `ABC` metric measures the size of a source code by counting
/// the number of Assignments (`A`), Branches (`B`) and Conditions (`C`).
/// The metric defines an ABC score as a vector of three elements (`<A,B,C>`).
/// The ABC score can be represented by its individual components (`A`, `B` and `C`)
/// or by the magnitude of the vector (`|<A,B,C>| = sqrt(A^2 + B^2 + C^2)`).
///
/// Official paper and definition:
///
/// Fitzpatrick, Jerry (1997). "Applying the ABC metric to C, C++ and Java". C++ Report.
///
/// <https://www.softwarerenovation.com/Articles.aspx>
#[derive(Debug, Clone)]
pub struct Stats {
    assignments: f64,
    assignments_sum: f64,
    assignments_min: f64,
    assignments_max: f64,
    branches: f64,
    branches_sum: f64,
    branches_min: f64,
    branches_max: f64,
    conditions: f64,
    conditions_sum: f64,
    conditions_min: f64,
    conditions_max: f64,
    space_count: usize,
    /// Whether a language actually counted anything into this.
    ///
    /// No language implements [`Abc`] today — the metric came from upstream
    /// with the Java and C++ grammars, which this fork does not carry — so
    /// without this flag every report would carry a full set of zeros that
    /// looks computed. [`Wmc`](crate::wmc), [`Npm`](crate::npm) and
    /// [`Npa`](crate::npa) hide themselves the same way.
    computed: bool,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            assignments: 0.,
            assignments_sum: 0.,
            assignments_min: f64::MAX,
            assignments_max: 0.,
            branches: 0.,
            branches_sum: 0.,
            branches_min: f64::MAX,
            branches_max: 0.,
            conditions: 0.,
            conditions_sum: 0.,
            conditions_min: f64::MAX,
            conditions_max: 0.,
            space_count: 1,
            computed: false,
        }
    }
}

impl Serialize for Stats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut st = serializer.serialize_struct("abc", 13)?;
        st.serialize_field("assignments", &self.assignments_sum())?;
        st.serialize_field("branches", &self.branches_sum())?;
        st.serialize_field("conditions", &self.conditions_sum())?;
        st.serialize_field("magnitude", &self.magnitude_sum())?;
        st.serialize_field("assignments_average", &self.assignments_average())?;
        st.serialize_field("branches_average", &self.branches_average())?;
        st.serialize_field("conditions_average", &self.conditions_average())?;
        st.serialize_field("assignments_min", &self.assignments_min())?;
        st.serialize_field("assignments_max", &self.assignments_max())?;
        st.serialize_field("branches_min", &self.branches_min())?;
        st.serialize_field("branches_max", &self.branches_max())?;
        st.serialize_field("conditions_min", &self.conditions_min())?;
        st.serialize_field("conditions_max", &self.conditions_max())?;
        st.end()
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "assignments: {}, branches: {}, conditions: {}, magnitude: {}, \
            assignments_average: {}, branches_average: {}, conditions_average: {}, \
            assignments_min: {}, assignments_max: {}, \
            branches_min: {}, branches_max: {}, \
            conditions_min: {}, conditions_max: {}",
            self.assignments_sum(),
            self.branches_sum(),
            self.conditions_sum(),
            self.magnitude_sum(),
            self.assignments_average(),
            self.branches_average(),
            self.conditions_average(),
            self.assignments_min(),
            self.assignments_max(),
            self.branches_min(),
            self.branches_max(),
            self.conditions_min(),
            self.conditions_max()
        )
    }
}

impl Stats {
    /// Merges a second `Abc` metric into the first one.
    pub fn merge(&mut self, other: &Stats) {
        // Calculates minimum and maximum values
        self.assignments_min = self.assignments_min.min(other.assignments_min);
        self.assignments_max = self.assignments_max.max(other.assignments_max);
        self.branches_min = self.branches_min.min(other.branches_min);
        self.branches_max = self.branches_max.max(other.branches_max);
        self.conditions_min = self.conditions_min.min(other.conditions_min);
        self.conditions_max = self.conditions_max.max(other.conditions_max);

        self.assignments_sum += other.assignments_sum;
        self.branches_sum += other.branches_sum;
        self.conditions_sum += other.conditions_sum;

        self.space_count += other.space_count;
        self.computed |= other.computed;
    }

    /// Marks the metric as one this language really counts.
    ///
    /// An implementation of [`Abc`] must call this, or its results stay
    /// hidden. See the `computed` field.
    #[inline(always)]
    #[expect(
        dead_code,
        reason = "the seam a real Abc implementation calls; no language has one yet"
    )]
    pub(crate) fn enable(&mut self) {
        self.computed = true;
    }

    /// Checks if the `Abc` metric is disabled.
    #[inline(always)]
    pub(crate) fn is_disabled(&self) -> bool {
        !self.computed
    }

    /// Returns the `Abc` assignments metric value.
    pub fn assignments(&self) -> f64 {
        self.assignments
    }

    /// Returns the `Abc` assignments sum metric value.
    pub fn assignments_sum(&self) -> f64 {
        self.assignments_sum
    }

    /// Returns the `Abc` assignments average value.
    ///
    /// This value is computed dividing the `Abc`
    /// assignments value for the number of spaces.
    pub fn assignments_average(&self) -> f64 {
        self.assignments_sum() / self.space_count as f64
    }

    /// Returns the `Abc` assignments minimum value.
    pub fn assignments_min(&self) -> f64 {
        self.assignments_min
    }

    /// Returns the `Abc` assignments maximum value.
    pub fn assignments_max(&self) -> f64 {
        self.assignments_max
    }

    /// Returns the `Abc` branches metric value.
    pub fn branches(&self) -> f64 {
        self.branches
    }

    /// Returns the `Abc` branches sum metric value.
    pub fn branches_sum(&self) -> f64 {
        self.branches_sum
    }

    /// Returns the `Abc` branches average value.
    ///
    /// This value is computed dividing the `Abc`
    /// branches value for the number of spaces.
    pub fn branches_average(&self) -> f64 {
        self.branches_sum() / self.space_count as f64
    }

    /// Returns the `Abc` branches minimum value.
    pub fn branches_min(&self) -> f64 {
        self.branches_min
    }

    /// Returns the `Abc` branches maximum value.
    pub fn branches_max(&self) -> f64 {
        self.branches_max
    }

    /// Returns the `Abc` conditions metric value.
    pub fn conditions(&self) -> f64 {
        self.conditions
    }

    /// Returns the `Abc` conditions sum metric value.
    pub fn conditions_sum(&self) -> f64 {
        self.conditions_sum
    }

    /// Returns the `Abc` conditions average value.
    ///
    /// This value is computed dividing the `Abc`
    /// conditions value for the number of spaces.
    pub fn conditions_average(&self) -> f64 {
        self.conditions_sum() / self.space_count as f64
    }

    /// Returns the `Abc` conditions minimum value.
    pub fn conditions_min(&self) -> f64 {
        self.conditions_min
    }

    /// Returns the `Abc` conditions maximum value.
    pub fn conditions_max(&self) -> f64 {
        self.conditions_max
    }

    /// Returns the `Abc` magnitude metric value.
    pub fn magnitude(&self) -> f64 {
        (self.assignments.powi(2) + self.branches.powi(2) + self.conditions.powi(2)).sqrt()
    }

    /// Returns the `Abc` magnitude sum metric value.
    pub fn magnitude_sum(&self) -> f64 {
        (self.assignments_sum.powi(2) + self.branches_sum.powi(2) + self.conditions_sum.powi(2))
            .sqrt()
    }

    #[inline(always)]
    pub(crate) fn compute_sum(&mut self) {
        self.assignments_sum += self.assignments;
        self.branches_sum += self.branches;
        self.conditions_sum += self.conditions;
    }

    #[inline(always)]
    pub(crate) fn compute_minmax(&mut self) {
        self.assignments_min = self.assignments_min.min(self.assignments);
        self.assignments_max = self.assignments_max.max(self.assignments);
        self.branches_min = self.branches_min.min(self.branches);
        self.branches_max = self.branches_max.max(self.branches);
        self.conditions_min = self.conditions_min.min(self.conditions);
        self.conditions_max = self.conditions_max.max(self.conditions);
        self.compute_sum();
    }
}

pub trait Abc
where
    Self: Checker,
{
    /// Counts assignments, branches and conditions into `stats`.
    ///
    /// An implementation must call `Stats::enable`, or what it counts is
    /// treated as never having been computed and stays out of the report.
    fn compute(node: &Node, stats: &mut Stats);
}

implement_metric_trait!(
    Abc,
    PythonCode,
    JavascriptCode,
    TypescriptCode,
    TsxCode,
    RustCode
);

// Fitzpatrick, Jerry (1997). "Applying the ABC metric to C, C++ and Java". C++ Report.
// Source: https://www.softwarerenovation.com/Articles.aspx
// ABC Java rules: (page 8, figure 4)
// ABC Java example: (page 15, listing 4)
