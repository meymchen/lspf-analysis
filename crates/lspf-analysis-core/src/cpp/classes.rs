//! File-local member inventories. A declaration owns a method; a matching
//! definition supplies its complexity without adding another method.
use std::collections::HashMap;

use crate::checker::Checker;
use crate::cyclomatic::Cyclomatic;
use crate::{Cpp, CppCode, FuncSpace, Node};

#[derive(Default)]
struct Class {
    attributes: usize,
    public_attributes: usize,
    methods: HashMap<String, Vec<Method>>,
}

struct Method {
    public: bool,
    complexity: Option<f64>,
}

fn text(node: Node<'_>, code: &[u8]) -> String {
    String::from_utf8_lossy(&code[node.start_byte()..node.end_byte()])
        .split_whitespace()
        .collect()
}

fn is_class(node: Node<'_>) -> bool {
    matches!(
        Cpp::from(node.kind_id()),
        Cpp::ClassSpecifier | Cpp::StructSpecifier | Cpp::UnionSpecifier
    ) && node.child_by_field_name("body").is_some()
}

fn injected_union(node: Node<'_>) -> bool {
    node.kind_id() == Cpp::UnionSpecifier as u16
        && node.child_by_field_name("name").is_none()
        && node.parent().is_some_and(|p| {
            p.kind_id() == Cpp::FieldDeclaration as u16
                && p.child_by_field_name("declarator").is_none()
        })
}

fn scope(node: Node<'_>, code: &[u8]) -> String {
    let mut parts = Vec::new();
    let mut parent = node.parent();
    while let Some(node) = parent {
        if is_class(node) || node.kind_id() == Cpp::NamespaceDefinition as u16 {
            parts.push(
                node.child_by_field_name("name")
                    .map(|n| text(n, code))
                    .unwrap_or_else(|| format!("@{}", node.id())),
            );
        } else if CppCode::is_func(&node) || CppCode::is_closure(&node) {
            parts.push(format!("@{}", node.id()));
        }
        parent = node.parent();
    }
    parts.reverse();
    parts.join("::")
}

fn qualified(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.to_owned()
    } else {
        format!("{scope}::{name}")
    }
}

/// Identify the operator closest to the name: `(*callback)(int)` is data,
/// whereas `*method(int)` and `(*factory(int))(int)` declare methods.
pub(crate) fn method_declarator(mut node: Node<'_>) -> Option<Node<'_>> {
    let mut operator = None;
    loop {
        match Cpp::from(node.kind_id()) {
            Cpp::FunctionDeclarator
            | Cpp::FunctionDeclarator2
            | Cpp::FunctionDeclarator3
            | Cpp::AbstractFunctionDeclarator => operator = Some(node),
            Cpp::PointerDeclarator
            | Cpp::PointerDeclarator2
            | Cpp::PointerTypeDeclarator
            | Cpp::ReferenceDeclarator
            | Cpp::ReferenceDeclarator2
            | Cpp::ReferenceDeclarator3
            | Cpp::ReferenceDeclarator4
            | Cpp::ArrayDeclarator
            | Cpp::ArrayDeclarator2
            | Cpp::ArrayDeclarator3 => operator = None,
            _ => {}
        }
        let Some(next) = super::declarator(node).or_else(|| node.child_by_field_name("name"))
        else {
            return operator;
        };
        node = next;
    }
}

fn method_name(mut node: Node<'_>, code: &[u8]) -> String {
    loop {
        if node.kind_id() == Cpp::OperatorCast as u16
            && let Some(parameters) =
                method_declarator(node).and_then(|n| n.child_by_field_name("parameters"))
        {
            return String::from_utf8_lossy(&code[node.start_byte()..parameters.start_byte()])
                .split_whitespace()
                .collect();
        }
        if matches!(
            Cpp::from(node.kind_id()),
            Cpp::Identifier
                | Cpp::FieldIdentifier
                | Cpp::DestructorName
                | Cpp::OperatorName
                | Cpp::TemplateMethod
                | Cpp::TemplateFunction
        ) {
            return text(node, code);
        }
        let Some(next) = super::declarator(node).or_else(|| node.child_by_field_name("name"))
        else {
            return text(node, code);
        };
        node = next;
    }
}

fn strip_template_arguments(name: &str) -> String {
    let mut depth = 0usize;
    name.chars()
        .filter(|c| match c {
            '<' => {
                depth += 1;
                false
            }
            '>' if depth > 0 => {
                depth -= 1;
                false
            }
            _ => depth == 0,
        })
        .collect()
}

/// Parameter names/defaults are not part of overload identity. Retain all type
/// tokens, pointer/reference operators, and cv/ref qualifiers on the method.
fn signature(declarator: Node<'_>, name: &str, code: &[u8]) -> String {
    fn tokens(node: Node<'_>, code: &[u8], out: &mut Vec<String>) {
        if node.kind_id() == Cpp::Comment as u16 {
            return;
        }
        if matches!(
            Cpp::from(node.kind_id()),
            Cpp::ParameterDeclaration
                | Cpp::OptionalParameterDeclaration
                | Cpp::VariadicParameterDeclaration
        ) {
            let name = super::function_name(node).map(|n| n.id());
            let default = node.child_by_field_name("default_value").map(|n| n.id());
            fn visit(
                node: Node<'_>,
                name: Option<usize>,
                default: Option<usize>,
                code: &[u8],
                out: &mut Vec<String>,
            ) {
                if Some(node.id()) == name
                    || Some(node.id()) == default
                    || node.kind_id() == Cpp::EQ as u16
                    || node.kind_id() == Cpp::Comment as u16
                {
                    return;
                }
                if node.child_count() == 0 {
                    out.push(text(node, code));
                } else {
                    for child in node.children() {
                        visit(child, name, default, code, out);
                    }
                }
            }
            visit(node, name, default, code, out);
        } else if node.child_count() == 0 {
            out.push(text(node, code));
        } else {
            for child in node.children() {
                tokens(child, code, out);
            }
        }
    }
    let mut parts = vec![name.to_owned()];
    if let Some(parameters) = declarator.child_by_field_name("parameters") {
        let mut parameter_tokens = Vec::new();
        tokens(parameters, code, &mut parameter_tokens);
        if parameter_tokens == ["(", "void", ")"] {
            parameter_tokens.remove(1);
        }
        parts.extend(parameter_tokens);
    }
    for child in declarator.children() {
        if matches!(
            Cpp::from(child.kind_id()),
            Cpp::TypeQualifier | Cpp::RefQualifier | Cpp::RequiresClause
        ) {
            tokens(child, code, &mut parts);
        }
    }
    parts.join(" ")
}

fn complexity(definition: Node<'_>) -> Option<f64> {
    let body = definition.child_by_field_name("body")?;
    if body.has_error() {
        return None;
    }
    let mut stats = crate::cyclomatic::Stats::default();
    let mut pending = vec![definition];
    while let Some(node) = pending.pop() {
        if node.id() != definition.id() && CppCode::is_func_space(&node) {
            continue;
        }
        CppCode::compute(&node, &mut stats);
        pending.extend(node.children());
    }
    Some(stats.cyclomatic())
}

fn method_complexity(member: Node<'_>, code: &[u8]) -> Option<f64> {
    complexity(member).or_else(|| {
        // These forms contain no executable source body.
        let specified = member.children().any(|n| {
            matches!(
                Cpp::from(n.kind_id()),
                Cpp::DefaultMethodClause | Cpp::DeleteMethodClause | Cpp::PureVirtualClause
            )
        });
        let pure = member
            .child_by_field_name("default_value")
            .is_some_and(|n| text(n, code) == "0");
        (specified || pure).then_some(0.)
    })
}

fn inventory(node: Node<'_>, code: &[u8]) -> Class {
    let mut class = Class::default();
    let mut public = node.kind_id() != Cpp::ClassSpecifier as u16;
    let mut pending: Vec<_> = node
        .child_by_field_name("body")
        .unwrap()
        .children()
        .collect();
    pending.reverse();
    while let Some(member) = pending.pop() {
        let kind = Cpp::from(member.kind_id());
        if kind == Cpp::AccessSpecifier {
            public = member.children().any(|n| n.kind_id() == Cpp::Public as u16);
            continue;
        }
        if matches!(
            kind,
            Cpp::TemplateDeclaration
                | Cpp::PreprocIf
                | Cpp::PreprocIfdef
                | Cpp::PreprocElse
                | Cpp::PreprocElif
                | Cpp::PreprocElifdef
                | Cpp::PreprocIf2
                | Cpp::PreprocIfdef2
                | Cpp::PreprocElse2
                | Cpp::PreprocElif2
                | Cpp::PreprocElifdef2
                | Cpp::PreprocIf3
                | Cpp::PreprocIfdef3
                | Cpp::PreprocElse3
                | Cpp::PreprocElif3
                | Cpp::PreprocElifdef3
                | Cpp::PreprocIf4
                | Cpp::PreprocIfdef4
                | Cpp::PreprocElse4
                | Cpp::PreprocElif4
                | Cpp::PreprocElifdef4
        ) {
            let children: Vec<_> = member.children().collect();
            pending.extend(children.into_iter().rev());
            continue;
        }
        if !matches!(
            kind,
            Cpp::FieldDeclaration
                | Cpp::Declaration
                | Cpp::Declaration2
                | Cpp::Declaration3
                | Cpp::Declaration4
                | Cpp::FunctionDefinition
                | Cpp::FunctionDefinition2
                | Cpp::FunctionDefinition3
                | Cpp::FunctionDefinition4
        ) {
            continue;
        }
        if let Some(anonymous) = member
            .child_by_field_name("type")
            .filter(|n| injected_union(*n))
        {
            let members = inventory(anonymous, code);
            class.attributes += members.attributes;
            class.public_attributes += if public { members.attributes } else { 0 };
        }
        let mut cursor = member.0.walk();
        for raw in member.0.children_by_field_name("declarator", &mut cursor) {
            let declarator = Node(raw);
            if let Some(function) = method_declarator(declarator) {
                let name = method_name(declarator, code);
                let key = signature(function, &name, code);
                let body = method_complexity(member, code);
                class.methods.entry(key).or_default().push(Method {
                    public,
                    complexity: body,
                });
            } else {
                class.attributes += 1;
                class.public_attributes += usize::from(public);
            }
        }
    }
    class
}

pub(crate) fn compute(root: Node<'_>, code: &[u8], space: &mut FuncSpace) {
    let mut nodes = Vec::new();
    let mut spaces = Vec::new();
    let mut classes = HashMap::new();
    let mut names: HashMap<String, Vec<usize>> = HashMap::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        nodes.push(node);
        if CppCode::is_func_space(&node) {
            spaces.push(node.id());
        }
        if is_class(node) && !injected_union(node) {
            if let Some(name) = node.child_by_field_name("name") {
                names
                    .entry(qualified(&scope(node, code), &text(name, code)))
                    .or_default()
                    .push(node.id());
            }
            classes.insert(node.id(), inventory(node, code));
        }
        let children: Vec<_> = node.children().collect();
        pending.extend(children.into_iter().rev());
    }
    let mut definitions: HashMap<(usize, String), Option<f64>> = HashMap::new();
    for node in nodes {
        if !CppCode::is_func(&node) {
            continue;
        }
        let Some(name) = super::function_name(node) else {
            continue;
        };
        if !matches!(
            Cpp::from(name.kind_id()),
            Cpp::QualifiedIdentifier
                | Cpp::QualifiedIdentifier2
                | Cpp::QualifiedIdentifier3
                | Cpp::QualifiedIdentifier4
        ) {
            continue;
        }
        let absolute = text(name, code).starts_with("::");
        let mut current = name;
        let mut owners = Vec::new();
        while matches!(
            Cpp::from(current.kind_id()),
            Cpp::QualifiedIdentifier
                | Cpp::QualifiedIdentifier2
                | Cpp::QualifiedIdentifier3
                | Cpp::QualifiedIdentifier4
        ) {
            if let Some(owner) = current.child_by_field_name("scope") {
                owners.push(text(owner, code));
            }
            let Some(next) = current.child_by_field_name("name") else {
                break;
            };
            current = next;
        }
        let name = method_name(current, code);
        let relative_owner = owners.join("::");
        let owner = if absolute {
            relative_owner
        } else {
            qualified(&scope(node, code), &relative_owner)
        };
        let generic = node.parent().is_some_and(|p| {
            p.kind_id() == Cpp::TemplateDeclaration as u16
                && p.child_by_field_name("parameters")
                    .is_some_and(|n| n.children().any(|c| c.is_named()))
        });
        let generic_owner = strip_template_arguments(&owner);
        let Some(ids) = names
            .get(&owner)
            .or_else(|| generic.then(|| names.get(&generic_owner)).flatten())
        else {
            continue;
        };
        if ids.len() != 1 {
            continue;
        }
        let Some(function) = super::declarator(node).and_then(method_declarator) else {
            continue;
        };
        let key = signature(function, &name, code);
        definitions
            .entry((ids[0], key))
            .and_modify(|value| *value = None)
            .or_insert_with(|| method_complexity(node, code));
    }
    for ((id, key), complexity) in definitions {
        if let Some(methods) = classes.get_mut(&id).unwrap().methods.get_mut(&key)
            && methods.len() == 1
        {
            methods[0].complexity = complexity;
        }
    }
    fn apply(
        space: &mut FuncSpace,
        ids: &mut impl Iterator<Item = usize>,
        classes: &HashMap<usize, Class>,
    ) {
        let id = ids
            .next()
            .expect("C++ AST and metric spaces have the same traversal order");
        let class = classes.get(&id);
        let (mut known, mut unresolved) = (0., 0);
        if let Some(class) = class {
            for method in class.methods.values().flatten() {
                if let Some(value) = method.complexity {
                    known += value;
                } else {
                    unresolved += 1;
                }
            }
        }
        space.metrics.npa = crate::npa::Stats::cpp(
            class.map_or(0, |c| c.public_attributes),
            class.map_or(0, |c| c.attributes),
        );
        space.metrics.npm = crate::npm::Stats::cpp(
            class.map_or(0, |c| {
                c.methods.values().flatten().filter(|m| m.public).count()
            }),
            class.map_or(0, |c| c.methods.values().map(Vec::len).sum()),
        );
        space.metrics.wmc = crate::wmc::Stats::cpp(space.kind, known, unresolved);
        for child in &mut space.spaces {
            apply(child, ids, classes);
            space.metrics.npa.merge(&child.metrics.npa);
            space.metrics.npm.merge(&child.metrics.npm);
            space.metrics.wmc.merge(&child.metrics.wmc);
        }
    }
    apply(space, &mut spaces.into_iter(), &classes);
}
