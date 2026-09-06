//! Syntax-based C++ ABC. Relational expressions count once; boolean contexts
//! additionally count their unary operands, including operands of && and ||.
use super::Stats;
use crate::checker::Checker;
use crate::{Cpp, CppCode, Node};

fn relational(node: Node<'_>) -> bool {
    node.child_by_field_name("operator")
        .is_some_and(|operator| {
            matches!(
                Cpp::from(operator.kind_id()),
                Cpp::EQEQ
                    | Cpp::BANGEQ
                    | Cpp::LT
                    | Cpp::LTEQ
                    | Cpp::GT
                    | Cpp::GT2
                    | Cpp::GTEQ
                    | Cpp::NotEq
            )
        })
}

fn unary_conditions(node: Node<'_>) -> f64 {
    match Cpp::from(node.kind_id()) {
        Cpp::ConditionClause => node
            .child_by_field_name("value")
            .map_or(0., unary_conditions),
        Cpp::ParenthesizedExpression | Cpp::ParenthesizedExpression2 => node
            .children()
            .find(|n| n.is_named() && n.kind_id() != Cpp::Comment as u16)
            .map_or(0., unary_conditions),
        Cpp::UnaryExpression | Cpp::UnaryExpression2 => node
            .child_by_field_name("argument")
            .map_or(1., unary_conditions),
        Cpp::BinaryExpression | Cpp::BinaryExpression2 => {
            if relational(node) {
                return 0.;
            }
            if node.child_by_field_name("operator").is_some_and(|op| {
                matches!(
                    Cpp::from(op.kind_id()),
                    Cpp::AMPAMP | Cpp::PIPEPIPE | Cpp::And | Cpp::Or
                )
            }) {
                return node
                    .child_by_field_name("left")
                    .map_or(0., unary_conditions)
                    + node
                        .child_by_field_name("right")
                        .map_or(0., unary_conditions);
            }
            1.
        }
        _ => 1.,
    }
}

fn logical(node: Node<'_>) -> bool {
    matches!(
        Cpp::from(node.kind_id()),
        Cpp::BinaryExpression
            | Cpp::BinaryExpression2
            | Cpp::UnaryExpression
            | Cpp::UnaryExpression2
    ) && node.child_by_field_name("operator").is_some_and(|op| {
        matches!(
            Cpp::from(op.kind_id()),
            Cpp::AMPAMP | Cpp::PIPEPIPE | Cpp::And | Cpp::Or | Cpp::BANG | Cpp::Not
        )
    })
}

fn enclosing_boolean_test(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(
            Cpp::from(parent.kind_id()),
            Cpp::ParenthesizedExpression | Cpp::ParenthesizedExpression2 | Cpp::ConditionClause
        ) {
            node = parent;
            continue;
        }
        return logical(parent)
            || parent
                .child_by_field_name("condition")
                .is_some_and(|condition| condition.id() == node.id());
    }
    false
}

pub(super) fn compute(node: Node<'_>, stats: &mut Stats) {
    stats.enable();
    let kind = Cpp::from(node.kind_id());
    match kind {
        Cpp::AssignmentExpression | Cpp::AssignmentExpression2 | Cpp::UpdateExpression => {
            stats.assignments += 1.;
        }
        Cpp::InitDeclarator if node.child_by_field_name("value").is_some() => {
            stats.assignments += 1.;
        }
        Cpp::FieldDeclaration => {
            let mut cursor = node.0.walk();
            let mut method = false;
            if cursor.goto_first_child() {
                loop {
                    match cursor.field_name() {
                        Some("declarator") => {
                            method = crate::cpp::method_declarator(Node(cursor.node())).is_some()
                        }
                        Some("default_value") if !method => stats.assignments += 1.,
                        _ => {}
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        Cpp::CallExpression | Cpp::CallExpression2 | Cpp::NewExpression | Cpp::DeleteExpression => {
            stats.branches += 1.;
        }
        Cpp::BinaryExpression | Cpp::BinaryExpression2 if relational(node) => {
            stats.conditions += 1.
        }
        Cpp::ElseClause | Cpp::CaseStatement | Cpp::CatchClause => stats.conditions += 1.,
        Cpp::ConditionalExpression => stats.conditions += 1.,
        _ => {}
    }
    if matches!(
        kind,
        Cpp::IfStatement
            | Cpp::WhileStatement
            | Cpp::DoStatement
            | Cpp::ForStatement
            | Cpp::ConditionalExpression
    ) && let Some(condition) = node.child_by_field_name("condition")
    {
        stats.conditions += unary_conditions(condition);
    }
    if logical(node) && !enclosing_boolean_test(node) {
        stats.conditions += unary_conditions(node);
    }
}

pub(super) fn compute_source(node: Node<'_>, code: &[u8], stats: &mut Stats) {
    if node.kind_id() == Cpp::GotoStatement as u16 {
        let Some(label) = node.child_by_field_name("label") else {
            return;
        };
        let label = &code[label.start_byte()..label.end_byte()];
        let mut function = node;
        let mut depth = 0;
        while let Some(parent) = function.parent() {
            function = parent;
            if CppCode::is_func(&function) || CppCode::is_closure(&function) {
                break;
            }
            depth += usize::from(function.kind_id() == Cpp::CompoundStatement as u16);
        }
        let mut pending = vec![(function, 0)];
        while let Some((current, level)) = pending.pop() {
            if current.id() != function.id() && CppCode::is_func_space(&current) {
                continue;
            }
            if current.kind_id() == Cpp::LabeledStatement as u16
                && level > depth
                && current
                    .child_by_field_name("label")
                    .is_some_and(|n| &code[n.start_byte()..n.end_byte()] == label)
            {
                stats.branches += 1.;
                break;
            }
            let next = level + usize::from(current.kind_id() == Cpp::CompoundStatement as u16);
            pending.extend(current.children().map(|child| (child, next)));
        }
    }
    if node.kind_id() != Cpp::PreprocArg as u16
        || !node.parent().is_some_and(|p| {
            matches!(
                Cpp::from(p.kind_id()),
                Cpp::PreprocDef | Cpp::PreprocFunctionDef
            )
        })
    {
        return;
    }
    let body = String::from_utf8_lossy(&code[node.start_byte()..node.end_byte()])
        .replace("\\\r\n", " ")
        .replace("\\\n", " ");
    for (prefix, suffix) in [
        ("void __macro() { ", "; }"),
        ("void __macro() { auto __value = (", "); }"),
    ] {
        let source = format!("{prefix}{body}{suffix}");
        let tree = crate::node::Tree::new::<CppCode>(source.as_bytes());
        let root = tree.get_root();
        if root.has_error() {
            continue;
        }
        let mut pending = vec![root];
        while let Some(current) = pending.pop() {
            // Do not count the expression wrapper's synthetic initialization.
            if current.start_byte() >= prefix.len()
                && current.start_byte() < prefix.len() + body.len()
            {
                compute(current, stats);
                if current.kind_id() == Cpp::GotoStatement as u16 {
                    compute_source(current, source.as_bytes(), stats);
                }
            }
            pending.extend(current.children());
        }
        break;
    }
}
