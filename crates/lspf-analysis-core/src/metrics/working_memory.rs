use serde::Serialize;
use serde::ser::{SerializeStruct, Serializer};
use std::collections::HashSet;
use std::fmt;

use crate::checker::Checker;
use crate::*;

/// The `WorkingMemory` metric.
///
/// This metric counts the variables a reader has to keep in their working
/// memory at the busiest statement of a function. A statement costs
/// one for every distinct name it mentions, one for every variable bound
/// earlier in the function that is still used later on, and one for every
/// name appearing in the conditions that control whether the statement runs.
/// Human working memory holds roughly 5 to 9 items, so a function scoring
/// well above that cannot be read in one pass.
///
/// Names are counted once each: a statement mentioning `total` three times
/// still costs one, because the reader remembers one thing.
#[derive(Debug, Clone)]
pub struct Stats {
    wm: usize,
    wm_sum: usize,
    total_space_functions: usize,
    wm_min: usize,
    wm_max: usize,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            wm: 0,
            wm_sum: 0,
            total_space_functions: 1,
            wm_min: usize::MAX,
            wm_max: 0,
        }
    }
}

impl Serialize for Stats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut st = serializer.serialize_struct("working_memory", 4)?;
        st.serialize_field("sum", &self.wm_sum())?;
        st.serialize_field("average", &self.wm_average())?;
        st.serialize_field("min", &self.wm_min())?;
        st.serialize_field("max", &self.wm_max())?;
        st.end()
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "sum: {}, average: {} min: {}, max: {}",
            self.wm_sum(),
            self.wm_average(),
            self.wm_min(),
            self.wm_max()
        )
    }
}

impl Stats {
    /// Merges a second `WorkingMemory` metric into the first one
    pub fn merge(&mut self, other: &Stats) {
        self.wm_max = self.wm_max.max(other.wm_max);
        self.wm_min = self.wm_min.min(other.wm_min);
        self.wm_sum += other.wm_sum;
    }

    /// Returns the `WorkingMemory` metric value of this space
    pub fn wm(&self) -> f64 {
        self.wm as f64
    }
    /// Returns the `WorkingMemory` metric sum value
    pub fn wm_sum(&self) -> f64 {
        self.wm_sum as f64
    }
    /// Returns the `WorkingMemory` metric minimum value
    pub fn wm_min(&self) -> f64 {
        self.wm_min as f64
    }
    /// Returns the `WorkingMemory` metric maximum value
    pub fn wm_max(&self) -> f64 {
        self.wm_max as f64
    }

    /// Returns the `WorkingMemory` metric average value
    ///
    /// This value is computed dividing the `WorkingMemory` sum value
    /// for the total number of functions/closures in a space.
    ///
    /// If there are no functions in a code, its value is `NAN`.
    pub fn wm_average(&self) -> f64 {
        self.wm_sum() / self.total_space_functions as f64
    }
    #[inline(always)]
    pub(crate) fn compute_sum(&mut self) {
        self.wm_sum += self.wm;
    }
    #[inline(always)]
    pub(crate) fn compute_minmax(&mut self) {
        self.wm_max = self.wm_max.max(self.wm);
        self.wm_min = self.wm_min.min(self.wm);
        self.compute_sum();
    }
    pub(crate) fn finalize(&mut self, total_space_functions: usize) {
        self.total_space_functions = total_space_functions;
    }
}

/// Describes, for one grammar, the node kinds the working memory walk needs
/// to recognize. Every field holds tree-sitter node kind names.
pub(crate) struct LangSpec {
    /// Nodes whose direct named children are the statements a reader walks
    /// through one at a time.
    bodies: &'static [&'static str],
    /// Nodes naming something the reader has to hold on to.
    references: &'static [&'static str],
    /// Binding sites, paired with the field holding the bound pattern.
    declarations: &'static [(&'static str, &'static str)],
    /// Parameter lists: every name inside one is bound by the signature.
    parameter_lists: &'static [&'static str],
    /// Control-flow nodes, paired with the field holding their condition.
    conditions: &'static [(&'static str, &'static str)],
}

const RUST_SPEC: LangSpec = LangSpec {
    bodies: &["block"],
    references: &["identifier", "field_identifier"],
    declarations: &[("let_declaration", "pattern"), ("parameter", "pattern")],
    parameter_lists: &["parameters", "closure_parameters"],
    conditions: &[
        ("if_expression", "condition"),
        ("while_expression", "condition"),
        ("for_expression", "value"),
        ("match_expression", "value"),
    ],
};

const PYTHON_SPEC: LangSpec = LangSpec {
    bodies: &["block"],
    references: &["identifier"],
    declarations: &[("assignment", "left"), ("augmented_assignment", "left")],
    parameter_lists: &["parameters", "lambda_parameters"],
    conditions: &[
        ("if_statement", "condition"),
        ("elif_clause", "condition"),
        ("while_statement", "condition"),
        ("for_statement", "right"),
        ("match_statement", "subject"),
    ],
};

const JAVA_SPEC: LangSpec = LangSpec {
    // A constructor's statements sit under `constructor_body`, not `block`.
    bodies: &["block", "constructor_body"],
    references: &["identifier"],
    // A `formal_parameter`'s name is bound by the signature, so it is
    // collected through `parameter_lists` rather than listed here.
    declarations: &[
        ("variable_declarator", "name"),
        ("enhanced_for_statement", "name"),
    ],
    parameter_lists: &["formal_parameters", "inferred_parameters"],
    conditions: &[
        ("if_statement", "condition"),
        ("while_statement", "condition"),
        ("do_statement", "condition"),
        ("for_statement", "condition"),
        ("enhanced_for_statement", "value"),
        ("switch_expression", "condition"),
    ],
};

const JS_SPEC: LangSpec = LangSpec {
    bodies: &["statement_block"],
    references: &[
        "identifier",
        "property_identifier",
        "shorthand_property_identifier",
    ],
    declarations: &[
        ("variable_declarator", "name"),
        ("required_parameter", "pattern"),
        ("optional_parameter", "pattern"),
    ],
    parameter_lists: &["formal_parameters"],
    conditions: &[
        ("if_statement", "condition"),
        ("while_statement", "condition"),
        ("do_statement", "condition"),
        ("for_statement", "condition"),
        ("for_in_statement", "right"),
        ("switch_statement", "value"),
    ],
};

/// One statement of a function body, with the names it mentions and the
/// names it binds.
struct Statement<'a> {
    start_byte: usize,
    refs: HashSet<&'a [u8]>,
    decls: HashSet<&'a [u8]>,
    /// Names appearing in the conditions that control this statement.
    conditions: HashSet<&'a [u8]>,
}

#[inline(always)]
fn text<'a>(node: &Node, code: &'a [u8]) -> &'a [u8] {
    &code[node.start_byte()..node.end_byte()]
}

/// Collects every name below `root` that `pred` accepts, without descending
/// into a nested body: a statement is charged for the names it mentions
/// itself, not for the ones its nested block mentions on later lines.
fn collect_names<'a>(
    spec: &LangSpec,
    root: &Node<'a>,
    code: &'a [u8],
    kinds: &[&'static str],
    names: &mut HashSet<&'a [u8]>,
) {
    let mut stack = vec![*root];
    while let Some(node) = stack.pop() {
        if node.id() != root.id() && spec.bodies.contains(&node.kind()) {
            continue;
        }
        if kinds.contains(&node.kind()) {
            names.insert(text(&node, code));
        }
        stack.extend(node.children());
    }
}

/// Collects the names a declaration or parameter list binds.
fn collect_declared<'a>(
    spec: &LangSpec,
    node: &Node<'a>,
    code: &'a [u8],
    decls: &mut HashSet<&'a [u8]>,
) {
    if spec.parameter_lists.contains(&node.kind()) {
        collect_names(spec, node, code, spec.references, decls);
        return;
    }
    for (kind, field) in spec.declarations {
        if node.kind() == *kind
            && let Some(bound) = node.child_by_field_name(field)
        {
            collect_names(spec, &bound, code, spec.references, decls);
        }
    }
}

/// Walks a statement, recording the names it mentions and the names it binds.
fn scan_statement<'a>(
    spec: &LangSpec,
    root: &Node<'a>,
    code: &'a [u8],
) -> (HashSet<&'a [u8]>, HashSet<&'a [u8]>) {
    let mut refs = HashSet::new();
    let mut decls = HashSet::new();
    let mut stack = vec![*root];
    while let Some(node) = stack.pop() {
        if node.id() != root.id() && spec.bodies.contains(&node.kind()) {
            continue;
        }
        if spec.references.contains(&node.kind()) {
            refs.insert(text(&node, code));
        }
        collect_declared(spec, &node, code, &mut decls);
        stack.extend(node.children());
    }
    (refs, decls)
}

/// Collects the names in the conditions controlling `stmt`, by walking up to
/// the space root through the control-flow nodes that enclose it.
fn enclosing_conditions<'a>(
    spec: &LangSpec,
    stmt: &Node<'a>,
    root: &Node<'a>,
    code: &'a [u8],
) -> HashSet<&'a [u8]> {
    let mut names = HashSet::new();
    let mut current = *stmt;
    while current.id() != root.id() {
        let Some(parent) = current.parent() else {
            break;
        };
        for (kind, field) in spec.conditions {
            if parent.kind() == *kind
                && let Some(condition) = parent.child_by_field_name(field)
            {
                collect_names(spec, &condition, code, spec.references, &mut names);
            }
        }
        current = parent;
    }
    names
}

/// Computes the working memory of one function space.
pub(crate) fn compute_space<T: Checker>(
    spec: &LangSpec,
    root: &Node,
    code: &[u8],
    stats: &mut Stats,
) {
    // The signature binds names before the first statement runs.
    let mut parameters = HashSet::new();
    let mut bodies = Vec::new();
    let mut stack = vec![*root];
    while let Some(node) = stack.pop() {
        // A nested function or closure gets its own space, and its own
        // working memory: it is not part of this one.
        if node.id() != root.id() && (T::is_func(&node) || T::is_func_space(&node)) {
            continue;
        }
        if spec.parameter_lists.contains(&node.kind()) {
            collect_names(spec, &node, code, spec.references, &mut parameters);
            continue;
        }
        if spec.bodies.contains(&node.kind()) {
            bodies.push(node);
        }
        stack.extend(node.children());
    }

    let mut statements: Vec<Statement> = Vec::new();
    for body in &bodies {
        for child in body.children() {
            if !child.is_named() || T::is_comment(&child) {
                continue;
            }
            let (refs, decls) = scan_statement(spec, &child, code);
            statements.push(Statement {
                start_byte: child.start_byte(),
                refs,
                decls,
                conditions: enclosing_conditions(spec, &child, root, code),
            });
        }
    }
    statements.sort_unstable_by_key(|statement| statement.start_byte);

    let mut max = 0;
    for (index, statement) in statements.iter().enumerate() {
        let mut held: HashSet<&[u8]> = statement.refs.clone();
        held.extend(statement.conditions.iter());
        // A name bound earlier and used later has to be carried across this
        // statement, whether or not this statement mentions it.
        let carried = parameters
            .iter()
            .copied()
            .chain(
                statements[..index]
                    .iter()
                    .flat_map(|earlier| earlier.decls.iter().copied()),
            )
            .filter(|name| {
                statements[index + 1..]
                    .iter()
                    .any(|later| later.refs.contains(name))
            });
        held.extend(carried);
        max = max.max(held.len());
    }

    stats.wm = max;
}

/// The `WorkingMemory` metric trait.
pub trait WorkingMemory
where
    Self: Checker,
{
    /// Computes the working memory of the function space rooted at `node`.
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats);
}

impl WorkingMemory for RustCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&RUST_SPEC, node, code, stats);
    }
}

impl WorkingMemory for PythonCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&PYTHON_SPEC, node, code, stats);
    }
}

impl WorkingMemory for JavaCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&JAVA_SPEC, node, code, stats);
    }
}

impl WorkingMemory for JavascriptCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&JS_SPEC, node, code, stats);
    }
}

impl WorkingMemory for TypescriptCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&JS_SPEC, node, code, stats);
    }
}

impl WorkingMemory for TsxCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&JS_SPEC, node, code, stats);
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    fn wm(lang: &LANG, source: &str, path: &str) -> f64 {
        let space =
            get_function_spaces(lang, source.as_bytes().to_vec(), std::path::Path::new(path))
                .unwrap();
        space.metrics.working_memory.wm_max()
    }

    #[test]
    fn rust_counts_distinct_names_of_the_busiest_statement() {
        // The busiest statement mentions a, b, c and total.
        let source = "fn f(a: u32, b: u32, c: u32) -> u32 {
    let total = a + b + c;
    total
}
";
        assert_eq!(wm(&LANG::Rust, source, "f.rs"), 4.0);
    }

    #[test]
    fn rust_repeating_a_name_costs_one() {
        let single = "fn f(a: u32) -> u32 {
    let x = a;
    x
}
";
        let repeated = "fn f(a: u32) -> u32 {
    let x = a + a + a + a;
    x
}
";
        assert_eq!(
            wm(&LANG::Rust, single, "f.rs"),
            wm(&LANG::Rust, repeated, "f.rs")
        );
    }

    #[test]
    fn rust_carries_a_variable_declared_above_and_used_below() {
        // `early` is bound first and mentioned last, so the middle statement
        // costs two even though it names only `middle`.
        let carried = "fn f() -> u32 {
    let early = 1;
    let middle = 2;
    early + middle
}
";
        // Here nothing has to be carried past the middle statement: `early`
        // is never mentioned again.
        let dropped = "fn f() -> u32 {
    let early = 1;
    let middle = 2;
    middle
}
";
        assert_eq!(wm(&LANG::Rust, carried, "f.rs"), 2.0);
        assert_eq!(wm(&LANG::Rust, dropped, "f.rs"), 1.0);
    }

    #[test]
    fn rust_charges_the_enclosing_condition() {
        let source = "fn f(flag: bool, value: u32) -> u32 {
    if flag {
        return value;
    }
    0
}
";
        // The `return value;` line mentions `value` and runs only under `flag`.
        assert_eq!(wm(&LANG::Rust, source, "f.rs"), 2.0);
    }

    #[test]
    fn rust_nested_function_is_not_charged_to_its_parent() {
        let outer = "fn f() -> u32 {
    fn inner() -> u32 {
        let a = 1;
        let b = 2;
        let c = 3;
        a + b + c
    }
    inner()
}
";
        // The outer function only ever holds `inner`.
        let space = get_function_spaces(
            &LANG::Rust,
            outer.as_bytes().to_vec(),
            std::path::Path::new("f.rs"),
        )
        .unwrap();
        let outer_space = space
            .spaces
            .iter()
            .find(|s| s.name.as_deref() == Some("f"))
            .unwrap();
        assert_eq!(outer_space.metrics.working_memory.wm(), 1.0);
    }

    #[test]
    fn python_counts_distinct_names() {
        let source = "def f(a, b, c):
    total = a + b + c
    return total
";
        assert_eq!(wm(&LANG::Python, source, "f.py"), 4.0);
    }

    #[test]
    fn python_charges_the_enclosing_condition() {
        let source = "def f(flag, value):
    if flag:
        return value
    return 0
";
        assert_eq!(wm(&LANG::Python, source, "f.py"), 2.0);
    }

    #[test]
    fn javascript_counts_distinct_names() {
        let source = "function f(a, b, c) {
    const total = a + b + c;
    return total;
}
";
        assert_eq!(wm(&LANG::Javascript, source, "f.js"), 4.0);
    }

    #[test]
    fn typescript_counts_distinct_names() {
        let source = "function f(a: number, b: number, c: number): number {
    const total = a + b + c;
    return total;
}
";
        assert_eq!(wm(&LANG::Typescript, source, "f.ts"), 4.0);
    }

    #[test]
    fn tsx_counts_distinct_names() {
        let source = "function f(a: number, b: number, c: number): number {
    const total = a + b + c;
    return total;
}
";
        assert_eq!(wm(&LANG::Tsx, source, "f.tsx"), 4.0);
    }

    #[test]
    fn an_empty_function_holds_nothing() {
        assert_eq!(wm(&LANG::Rust, "fn f() {}\n", "f.rs"), 0.0);
    }
}
