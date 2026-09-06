//! Conservative, file-local call resolution. Ambiguous bindings never add edges.
//! Qualified calls and broader binding resolution: https://github.com/meymchen/lspf-analysis/issues/1.

use std::collections::{HashMap, HashSet};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::{LANG, Node};

mod node_kind;
use node_kind::{Kind, kind};

#[derive(Default)]
struct Scope<'a> {
    parent: Option<usize>,
    bindings: HashMap<&'a [u8], Option<NodeIndex>>,
    class: bool,
    opaque: bool,
}

impl<'a> Scope<'a> {
    fn bind(&mut self, name: &'a [u8], function: Option<NodeIndex>) {
        self.bindings
            .entry(name)
            .and_modify(|binding| *binding = None)
            .or_insert(function);
    }
}

fn text<'a>(node: Node, code: &'a [u8]) -> &'a [u8] {
    &code[node.start_byte()..node.end_byte()]
}

fn is_function(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::FunctionItem
            | Kind::FunctionDefinition
            | Kind::FunctionDeclaration
            | Kind::GeneratorFunctionDeclaration
            | Kind::FunctionExpression
            | Kind::GeneratorFunction
            | Kind::MethodDeclaration
            | Kind::MethodDefinition
            | Kind::ConstructorDeclaration
            | Kind::ArrowFunction
            | Kind::ClosureExpression
            | Kind::Lambda
            | Kind::LambdaExpression
    )
}

fn is_class(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ClassBody
            | Kind::ClassDefinition
            | Kind::ImplItem
            | Kind::TraitItem
            | Kind::ClassSpecifier
            | Kind::StructSpecifier
            | Kind::UnionSpecifier
    )
}

fn is_scope(kind: Kind, language: LANG) -> bool {
    is_function(kind)
        || is_class(kind)
        || matches!(
            kind,
            Kind::DeclarationList | Kind::ModItem | Kind::NamespaceDefinition
        )
        || (!matches!(language, LANG::Python)
            && matches!(
                kind,
                Kind::Block
                    | Kind::CompoundStatement
                    | Kind::ForRangeLoop
                    | Kind::StatementBlock
                    | Kind::ForStatement
                    | Kind::ForInStatement
                    | Kind::ForExpression
                    | Kind::CatchClause
                    | Kind::MatchArm
                    | Kind::IfExpression
                    | Kind::WhileExpression
            ))
}

fn callable(node: Node, language: LANG) -> bool {
    if node.has_error() || node.child_by_field_name("body").is_none() {
        return false;
    }
    match language {
        LANG::Java => {
            kind(node, language) == Kind::MethodDeclaration
                && node.children().any(|child| {
                    kind(child, language) == Kind::Modifiers
                        && child.children().any(|modifier| {
                            matches!(
                                kind(modifier, language),
                                Kind::Static | Kind::Private | Kind::Final
                            )
                        })
                })
        }
        LANG::Python => {
            kind(node, language) == Kind::FunctionDefinition
                && !node
                    .parent()
                    .is_some_and(|parent| kind(parent, language) == Kind::DecoratedDefinition)
        }
        LANG::Rust => kind(node, language) == Kind::FunctionItem,
        LANG::Cpp => {
            kind(node, language) == Kind::FunctionDefinition
                && crate::cpp::function_name(node)
                    .is_some_and(|n| kind(n, language) == Kind::Identifier)
        }
        _ => matches!(
            kind(node, language),
            Kind::FunctionDeclaration
                | Kind::GeneratorFunctionDeclaration
                | Kind::FunctionExpression
                | Kind::GeneratorFunction
        ),
    }
}

// Only binding patterns are visited, never their type annotations or default values.
fn bind_pattern<'a>(root: Node, code: &'a [u8], scope: &mut Scope<'a>, language: LANG) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let node_kind = kind(node, language);
        if matches!(
            node_kind,
            Kind::Identifier | Kind::ShorthandPropertyIdentifierPattern
        ) {
            scope.bind(text(node, code), None);
            continue;
        }
        if matches!(
            node_kind,
            Kind::Attribute | Kind::MemberExpression | Kind::Subscript | Kind::SubscriptExpression
        ) {
            continue;
        }
        let bound = node
            .child_by_field_name("declarator")
            .or_else(|| node.child_by_field_name("pattern"))
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.child_by_field_name("left"));
        if let Some(bound) = bound {
            pending.push(bound);
        } else if node_kind == Kind::PairPattern {
            pending.extend(node.child_by_field_name("value"));
        } else {
            pending.extend(node.children().filter(|child| {
                child.is_named()
                    && !matches!(
                        kind(*child, language),
                        Kind::TypeAnnotation | Kind::TypeIdentifier
                    )
            }));
        }
    }
}

fn binding_field(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Declaration
        | Kind::InitDeclarator
        | Kind::ParameterDeclaration
        | Kind::OptionalParameterDeclaration => Some("declarator"),
        Kind::LetDeclaration | Kind::LetCondition | Kind::ForExpression | Kind::MatchArm => {
            Some("pattern")
        }
        Kind::VariableDeclarator
        | Kind::EnhancedForStatement
        | Kind::ConstItem
        | Kind::StaticItem
        | Kind::StructItem
        | Kind::EnumItem => Some("name"),
        Kind::Assignment
        | Kind::AugmentedAssignment
        | Kind::AssignmentExpression
        | Kind::AugmentedAssignmentExpression
        | Kind::ForInStatement => Some("left"),
        Kind::ForStatement => Some("left"),
        Kind::CatchClause => Some("parameter"),
        _ => None,
    }
}

fn resolve(scopes: &[Scope], mut scope: usize, name: &[u8], language: LANG) -> Option<NodeIndex> {
    loop {
        let current = &scopes[scope];
        if current.opaque {
            return None;
        }
        // Python class members and Rust associated functions are not lexical locals.
        if !(current.class && matches!(language, LANG::Python | LANG::Rust))
            && let Some(binding) = current.bindings.get(name)
        {
            return *binding;
        }
        scope = current.parent?;
    }
}

/// Returns AST identities of functions in statically resolved recursion cycles.
pub(super) fn functions(root: Node, code: &[u8], language: LANG) -> HashSet<usize> {
    let mut graph = DiGraph::<usize, ()>::new();
    let mut java_arities = HashMap::new();
    let mut scopes = vec![Scope::default()];
    let mut calls = Vec::new();
    let mut pending = vec![(root, 0, None)];
    while let Some((node, mut scope, mut owner)) = pending.pop() {
        let node_kind = kind(node, language);
        if matches!(node_kind, Kind::MacroDefinition | Kind::MacroInvocation) {
            continue;
        }
        if matches!(language, LANG::Cpp)
            && matches!(node_kind, Kind::PreprocDef | Kind::PreprocFunctionDef)
        {
            if let Some(name) = node.child_by_field_name("name") {
                scopes[scope].bind(text(name, code), None);
            }
            continue;
        }
        let function = is_function(node_kind);
        let target = if function && callable(node, language) {
            Some(graph.add_node(node.id()))
        } else {
            None
        };
        if matches!(language, LANG::Java)
            && let Some(target) = target
        {
            let parameters = node.child_by_field_name("parameters");
            let arity = parameters
                .filter(|parameters| {
                    !parameters
                        .children()
                        .any(|parameter| kind(parameter, language) == Kind::SpreadParameter)
                })
                .map(|parameters| {
                    parameters
                        .children()
                        .filter(|parameter| {
                            parameter.is_named()
                                && !matches!(
                                    kind(*parameter, language),
                                    Kind::LineComment | Kind::BlockComment
                                )
                        })
                        .count()
                });
            java_arities.insert(target, arity);
        }
        let private_name = matches!(
            node_kind,
            Kind::FunctionExpression | Kind::GeneratorFunction
        );
        if (function || node_kind == Kind::ClassDefinition || node_kind == Kind::ClassDeclaration)
            && !private_name
            && let Some(name) = node.child_by_field_name("name").or_else(|| {
                matches!(language, LANG::Cpp)
                    .then(|| crate::cpp::function_name(node))
                    .flatten()
            })
        {
            scopes[scope].bind(text(name, code), target);
        }
        if is_scope(node_kind, language) {
            let next = scopes.len();
            let class = is_class(node_kind)
                || (node_kind == Kind::DeclarationList
                    && node.parent().is_some_and(|parent| {
                        matches!(kind(parent, language), Kind::ImplItem | Kind::TraitItem)
                    }));
            scopes.push(Scope {
                parent: Some(scope),
                class,
                ..Scope::default()
            });
            scope = next;
        }
        if function {
            owner = target;
            if private_name && let Some(name) = node.child_by_field_name("name") {
                scopes[scope].bind(text(name, code), target);
            }
            if let Some(parameters) = node.child_by_field_name("parameters").or_else(|| {
                matches!(language, LANG::Cpp)
                    .then(|| {
                        crate::cpp::function_declarator(node)
                            .and_then(|d| d.child_by_field_name("parameters"))
                    })
                    .flatten()
            }) {
                bind_pattern(parameters, code, &mut scopes[scope], language);
            }
            if let Some(parameter) = node.child_by_field_name("parameter") {
                bind_pattern(parameter, code, &mut scopes[scope], language);
            }
        }
        if let Some(field) = binding_field(node_kind)
            && let Some(pattern) = node.child_by_field_name(field)
        {
            // JS var declarations and assignments may affect enclosing bindings.
            // Blocking the name in every ancestor avoids inventing a recursive edge.
            let broad = matches!(
                node_kind,
                Kind::Assignment
                    | Kind::AugmentedAssignment
                    | Kind::AssignmentExpression
                    | Kind::AugmentedAssignmentExpression
            ) || node
                .parent()
                .is_some_and(|p| kind(p, language) == Kind::VariableDeclaration);
            let mut destination = scope;
            loop {
                bind_pattern(pattern, code, &mut scopes[destination], language);
                if !broad {
                    break;
                }
                let Some(parent) = scopes[destination].parent else {
                    break;
                };
                destination = parent;
            }
        }
        if matches!(
            node_kind,
            Kind::ImportStatement
                | Kind::ImportFromStatement
                | Kind::UseDeclaration
                | Kind::UsingDeclaration
        ) {
            let import = text(node, code);
            if import.contains(&b'*') {
                scopes[scope].opaque = true;
            } else {
                // Binding every identifier is conservative for aliases, but still
                // permits calls unrelated to the imported names.
                bind_pattern(node, code, &mut scopes[scope], language);
            }
        }
        if matches!(
            node_kind,
            Kind::GlobalStatement
                | Kind::NonlocalStatement
                | Kind::WithStatement
                | Kind::ExceptClause
                | Kind::ListComprehension
                | Kind::DictionaryComprehension
                | Kind::SetComprehension
                | Kind::GeneratorExpression
                | Kind::MatchStatement
                | Kind::DeleteStatement
        ) {
            // Wildcard imports and these binding forms need language-specific resolution.
            scopes[scope].opaque = true;
        }
        if let Some(owner) = owner {
            let callee = match node_kind {
                Kind::Call | Kind::CallExpression => node.child_by_field_name("function"),
                Kind::MethodInvocation if node.child_by_field_name("object").is_none() => {
                    node.child_by_field_name("name")
                }
                _ => None,
            };
            if let Some(callee) = callee
                && kind(callee, language) == Kind::Identifier
            {
                let name = text(callee, code);
                if name == b"eval"
                    && matches!(language, LANG::Javascript | LANG::Typescript | LANG::Tsx)
                {
                    scopes[scope].opaque = true;
                }
                let arity = node.child_by_field_name("arguments").map(|arguments| {
                    arguments
                        .children()
                        .filter(|argument| {
                            argument.is_named()
                                && !matches!(
                                    kind(*argument, language),
                                    Kind::LineComment | Kind::BlockComment
                                )
                        })
                        .count()
                });
                calls.push((owner, scope, name, arity));
            }
        }
        if function {
            // Defaults and annotations execute outside the function body.
            pending.extend(
                node.child_by_field_name("body")
                    .map(|body| (body, scope, owner)),
            );
        } else {
            pending.extend(node.children().map(|child| (child, scope, owner)));
        }
    }
    for (caller, scope, name, arity) in calls {
        if let Some(callee) = resolve(&scopes, scope, name, language) {
            if matches!(language, LANG::Java)
                && (arity.is_none() || java_arities.get(&callee) != Some(&arity))
            {
                continue;
            }
            graph.update_edge(caller, callee, ());
        }
    }
    kosaraju_scc(&graph)
        .into_iter()
        .filter(|component| component.len() > 1 || graph.contains_edge(component[0], component[0]))
        .flatten()
        .map(|index| graph[index])
        .collect()
}
