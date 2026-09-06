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
/// to recognize. Node categories use grammar-specific tree-sitter kind IDs.
pub(crate) struct LangSpec {
    /// Nodes whose direct named children are the statements a reader walks
    /// through one at a time.
    bodies: &'static [u16],
    /// Nodes naming something the reader has to hold on to.
    references: &'static [u16],
    /// Binding sites, paired with the field holding the bound pattern.
    declarations: &'static [(u16, &'static str)],
    /// Parameter lists: every name inside one is bound by the signature.
    parameter_lists: &'static [u16],
    /// Control-flow nodes, paired with the field holding their condition.
    conditions: &'static [(u16, &'static str)],
}

const RUST_SPEC: LangSpec = LangSpec {
    bodies: &[Rust::Block2 as u16, Rust::Block as u16],
    references: &[Rust::Identifier as u16, Rust::FieldIdentifier as u16],
    declarations: &[
        (Rust::LetDeclaration as u16, "pattern"),
        (Rust::Parameter as u16, "pattern"),
    ],
    parameter_lists: &[Rust::Parameters as u16, Rust::ClosureParameters as u16],
    conditions: &[
        (Rust::IfExpression as u16, "condition"),
        (Rust::WhileExpression as u16, "condition"),
        (Rust::ForExpression as u16, "value"),
        (Rust::MatchExpression as u16, "value"),
    ],
};

const PYTHON_SPEC: LangSpec = LangSpec {
    bodies: &[Python::Block as u16, Python::Block2 as u16],
    references: &[Python::Identifier as u16],
    declarations: &[
        (Python::Assignment as u16, "left"),
        (Python::AugmentedAssignment as u16, "left"),
    ],
    parameter_lists: &[Python::Parameters as u16, Python::LambdaParameters as u16],
    conditions: &[
        (Python::IfStatement as u16, "condition"),
        (Python::ElifClause as u16, "condition"),
        (Python::WhileStatement as u16, "condition"),
        (Python::ForStatement as u16, "right"),
        (Python::MatchStatement as u16, "subject"),
    ],
};

const JAVA_SPEC: LangSpec = LangSpec {
    // A constructor's statements sit under `constructor_body`, not `block`.
    bodies: &[Java::Block as u16, Java::ConstructorBody as u16],
    references: &[Java::Identifier as u16],
    // A `formal_parameter`'s name is bound by the signature, so it is
    // collected through `parameter_lists` rather than listed here.
    declarations: &[
        (Java::VariableDeclarator as u16, "name"),
        (Java::EnhancedForStatement as u16, "name"),
    ],
    parameter_lists: &[
        Java::FormalParameters as u16,
        Java::InferredParameters as u16,
    ],
    conditions: &[
        (Java::IfStatement as u16, "condition"),
        (Java::WhileStatement as u16, "condition"),
        (Java::DoStatement as u16, "condition"),
        (Java::ForStatement as u16, "condition"),
        (Java::EnhancedForStatement as u16, "value"),
        (Java::SwitchExpression as u16, "condition"),
    ],
};

const JS_SPEC: LangSpec = LangSpec {
    bodies: &[Javascript::StatementBlock as u16],
    references: &[
        Javascript::Identifier as u16,
        Javascript::Identifier2 as u16,
        Javascript::PropertyIdentifier as u16,
        Javascript::ShorthandPropertyIdentifier as u16,
    ],
    declarations: &[(Javascript::VariableDeclarator as u16, "name")],
    parameter_lists: &[Javascript::FormalParameters as u16],
    conditions: &[
        (Javascript::IfStatement as u16, "condition"),
        (Javascript::WhileStatement as u16, "condition"),
        (Javascript::DoStatement as u16, "condition"),
        (Javascript::ForStatement as u16, "condition"),
        (Javascript::ForInStatement as u16, "right"),
        (Javascript::SwitchStatement as u16, "value"),
    ],
};

const TS_SPEC: LangSpec = LangSpec {
    bodies: &[Typescript::StatementBlock as u16],
    references: &[
        Typescript::Identifier as u16,
        Typescript::PropertyIdentifier as u16,
        Typescript::ShorthandPropertyIdentifier as u16,
    ],
    declarations: &[
        (Typescript::VariableDeclarator as u16, "name"),
        (Typescript::RequiredParameter as u16, "pattern"),
        (Typescript::RequiredParameter2 as u16, "pattern"),
        (Typescript::OptionalParameter as u16, "pattern"),
        (Typescript::OptionalParameter2 as u16, "pattern"),
    ],
    parameter_lists: &[Typescript::FormalParameters as u16],
    conditions: &[
        (Typescript::IfStatement as u16, "condition"),
        (Typescript::WhileStatement as u16, "condition"),
        (Typescript::DoStatement as u16, "condition"),
        (Typescript::ForStatement as u16, "condition"),
        (Typescript::ForInStatement as u16, "right"),
        (Typescript::SwitchStatement as u16, "value"),
    ],
};

const TSX_SPEC: LangSpec = LangSpec {
    bodies: &[Tsx::StatementBlock as u16],
    references: &[
        Tsx::Identifier as u16,
        Tsx::Identifier2 as u16,
        Tsx::PropertyIdentifier as u16,
        Tsx::ShorthandPropertyIdentifier as u16,
    ],
    declarations: &[
        (Tsx::VariableDeclarator as u16, "name"),
        (Tsx::RequiredParameter as u16, "pattern"),
        (Tsx::RequiredParameter2 as u16, "pattern"),
        (Tsx::OptionalParameter as u16, "pattern"),
        (Tsx::OptionalParameter2 as u16, "pattern"),
    ],
    parameter_lists: &[Tsx::FormalParameters as u16],
    conditions: &[
        (Tsx::IfStatement as u16, "condition"),
        (Tsx::WhileStatement as u16, "condition"),
        (Tsx::DoStatement as u16, "condition"),
        (Tsx::ForStatement as u16, "condition"),
        (Tsx::ForInStatement as u16, "right"),
        (Tsx::SwitchStatement as u16, "value"),
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
    kinds: &[u16],
    names: &mut HashSet<&'a [u8]>,
) {
    let mut stack = vec![*root];
    while let Some(node) = stack.pop() {
        if node.id() != root.id() && spec.bodies.contains(&node.kind_id()) {
            continue;
        }
        if kinds.contains(&node.kind_id()) {
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
    if spec.parameter_lists.contains(&node.kind_id()) {
        collect_names(spec, node, code, spec.references, decls);
        return;
    }
    for (kind, field) in spec.declarations {
        if node.kind_id() == *kind
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
        if node.id() != root.id() && spec.bodies.contains(&node.kind_id()) {
            continue;
        }
        if spec.references.contains(&node.kind_id()) {
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
            if parent.kind_id() == *kind
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
        if spec.parameter_lists.contains(&node.kind_id()) {
            collect_names(spec, &node, code, spec.references, &mut parameters);
            continue;
        }
        if spec.bodies.contains(&node.kind_id()) {
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
        compute_space::<Self>(&TS_SPEC, node, code, stats);
    }
}

impl WorkingMemory for TsxCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        compute_space::<Self>(&TSX_SPEC, node, code, stats);
    }
}

impl WorkingMemory for CppCode {
    fn compute_space(node: &Node, code: &[u8], stats: &mut Stats) {
        const SPEC: LangSpec = LangSpec {
            bodies: &[Cpp::CompoundStatement as u16],
            references: &[Cpp::Identifier as u16, Cpp::FieldIdentifier as u16],
            declarations: &[
                (Cpp::InitDeclarator as u16, "declarator"),
                (Cpp::Declaration as u16, "declarator"),
                (Cpp::Declaration2 as u16, "declarator"),
                (Cpp::Declaration3 as u16, "declarator"),
                (Cpp::Declaration4 as u16, "declarator"),
                (Cpp::ForRangeLoop as u16, "declarator"),
            ],
            parameter_lists: &[Cpp::ParameterList as u16, Cpp::ParameterList2 as u16],
            conditions: &[
                (Cpp::IfStatement as u16, "condition"),
                (Cpp::WhileStatement as u16, "condition"),
                (Cpp::DoStatement as u16, "condition"),
                (Cpp::ForStatement as u16, "condition"),
                (Cpp::ForRangeLoop as u16, "right"),
                (Cpp::SwitchStatement as u16, "condition"),
            ],
        };
        compute_space::<Self>(&SPEC, node, code, stats);
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
