//! Conservative, file-local call resolution. Ambiguous bindings never add edges.
//! Qualified calls and broader binding resolution: https://github.com/meymchen/lspf-analysis/issues/1.

use std::collections::{HashMap, HashSet};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::{LANG, Node};

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

fn is_function(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_definition"
            | "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "method_declaration"
            | "method_definition"
            | "constructor_declaration"
            | "arrow_function"
            | "closure_expression"
            | "lambda"
            | "lambda_expression"
    )
}

fn is_class(kind: &str) -> bool {
    matches!(
        kind,
        "class_body" | "class_definition" | "impl_item" | "trait_item"
    )
}

fn is_scope(kind: &str, language: LANG) -> bool {
    is_function(kind)
        || is_class(kind)
        || matches!(kind, "declaration_list" | "mod_item")
        || (!matches!(language, LANG::Python)
            && matches!(
                kind,
                "block"
                    | "statement_block"
                    | "for_statement"
                    | "for_in_statement"
                    | "for_expression"
                    | "catch_clause"
                    | "match_arm"
                    | "if_expression"
                    | "while_expression"
            ))
}

fn callable(node: Node, language: LANG) -> bool {
    if node.has_error() || node.child_by_field_name("body").is_none() {
        return false;
    }
    match language {
        LANG::Java => {
            node.kind() == "method_declaration"
                && node.children().any(|child| {
                    child.kind() == "modifiers"
                        && child.children().any(|modifier| {
                            matches!(modifier.kind(), "static" | "private" | "final")
                        })
                })
        }
        LANG::Python => {
            node.kind() == "function_definition"
                && !node
                    .parent()
                    .is_some_and(|parent| parent.kind() == "decorated_definition")
        }
        LANG::Rust => node.kind() == "function_item",
        _ => matches!(
            node.kind(),
            "function_declaration"
                | "generator_function_declaration"
                | "function_expression"
                | "generator_function"
        ),
    }
}

// Only binding patterns are visited, never their type annotations or default values.
fn bind_pattern<'a>(root: Node, code: &'a [u8], scope: &mut Scope<'a>) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if matches!(
            node.kind(),
            "identifier" | "shorthand_property_identifier_pattern"
        ) {
            scope.bind(text(node, code), None);
            continue;
        }
        if matches!(
            node.kind(),
            "attribute" | "member_expression" | "subscript" | "subscript_expression"
        ) {
            continue;
        }
        let bound = node
            .child_by_field_name("pattern")
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.child_by_field_name("left"));
        if let Some(bound) = bound {
            pending.push(bound);
        } else if node.kind() == "pair_pattern" {
            pending.extend(node.child_by_field_name("value"));
        } else {
            pending.extend(node.children().filter(|child| {
                child.is_named() && !matches!(child.kind(), "type_annotation" | "type_identifier")
            }));
        }
    }
}

fn binding_field(kind: &str) -> Option<&'static str> {
    match kind {
        "let_declaration" | "let_condition" | "for_expression" | "match_arm" => Some("pattern"),
        "variable_declarator"
        | "enhanced_for_statement"
        | "const_item"
        | "static_item"
        | "struct_item"
        | "enum_item" => Some("name"),
        "assignment"
        | "augmented_assignment"
        | "assignment_expression"
        | "augmented_assignment_expression"
        | "for_in_statement" => Some("left"),
        "for_statement" => Some("left"),
        "catch_clause" => Some("parameter"),
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
        if matches!(node.kind(), "macro_definition" | "macro_invocation") {
            continue;
        }
        let function = is_function(node.kind());
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
                        .any(|parameter| parameter.kind() == "spread_parameter")
                })
                .map(|parameters| {
                    parameters
                        .children()
                        .filter(|parameter| {
                            parameter.is_named()
                                && !matches!(parameter.kind(), "line_comment" | "block_comment")
                        })
                        .count()
                });
            java_arities.insert(target, arity);
        }
        let private_name = matches!(node.kind(), "function_expression" | "generator_function");
        if (function || node.kind() == "class_definition" || node.kind() == "class_declaration")
            && !private_name
            && let Some(name) = node.child_by_field_name("name")
        {
            scopes[scope].bind(text(name, code), target);
        }
        if is_scope(node.kind(), language) {
            let next = scopes.len();
            let class = is_class(node.kind())
                || (node.kind() == "declaration_list"
                    && node
                        .parent()
                        .is_some_and(|parent| matches!(parent.kind(), "impl_item" | "trait_item")));
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
            if let Some(parameters) = node.child_by_field_name("parameters") {
                bind_pattern(parameters, code, &mut scopes[scope]);
            }
            if let Some(parameter) = node.child_by_field_name("parameter") {
                bind_pattern(parameter, code, &mut scopes[scope]);
            }
        }
        if let Some(field) = binding_field(node.kind())
            && let Some(pattern) = node.child_by_field_name(field)
        {
            // JS var declarations and assignments may affect enclosing bindings.
            // Blocking the name in every ancestor avoids inventing a recursive edge.
            let broad = matches!(
                node.kind(),
                "assignment"
                    | "augmented_assignment"
                    | "assignment_expression"
                    | "augmented_assignment_expression"
            ) || node
                .parent()
                .is_some_and(|p| p.kind() == "variable_declaration");
            let mut destination = scope;
            loop {
                bind_pattern(pattern, code, &mut scopes[destination]);
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
            node.kind(),
            "import_statement" | "import_from_statement" | "use_declaration"
        ) {
            let import = text(node, code);
            if import.contains(&b'*') {
                scopes[scope].opaque = true;
            } else {
                // Binding every identifier is conservative for aliases, but still
                // permits calls unrelated to the imported names.
                bind_pattern(node, code, &mut scopes[scope]);
            }
        }
        if matches!(
            node.kind(),
            "global_statement"
                | "nonlocal_statement"
                | "with_statement"
                | "except_clause"
                | "list_comprehension"
                | "dictionary_comprehension"
                | "set_comprehension"
                | "generator_expression"
                | "match_statement"
                | "delete_statement"
        ) {
            // Wildcard imports and these binding forms need language-specific resolution.
            scopes[scope].opaque = true;
        }
        if let Some(owner) = owner {
            let callee = match node.kind() {
                "call" | "call_expression" => node.child_by_field_name("function"),
                "method_invocation" if node.child_by_field_name("object").is_none() => {
                    node.child_by_field_name("name")
                }
                _ => None,
            };
            if let Some(callee) = callee
                && callee.kind() == "identifier"
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
                                && !matches!(argument.kind(), "line_comment" | "block_comment")
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
