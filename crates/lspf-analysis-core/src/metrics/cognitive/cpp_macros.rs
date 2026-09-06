//! Measure source-visible macro replacement lists once, at their definition.
//! Included definitions and build-dependent expansion are outside source metrics.
use std::collections::HashMap;

use super::{Cognitive, Stats};
use crate::{Cpp, CppCode, Node, node::Tree};

pub(super) fn compute(node: Node, code: &[u8], stats: &mut Stats, nesting: (usize, usize, usize)) {
    if !node.parent().is_some_and(|p| {
        matches!(
            Cpp::from(p.kind_id()),
            Cpp::PreprocDef | Cpp::PreprocFunctionDef
        )
    }) {
        return;
    }
    let body = String::from_utf8_lossy(&code[node.start_byte()..node.end_byte()])
        .replace("\\\r\n", " ")
        .replace("\\\n", " ");
    // Statement macros and expression macros both need a function context.
    for (prefix, suffix) in [
        ("void __macro() { ", "; }"),
        ("void __macro() { auto __value = (", "); }"),
    ] {
        let source = format!("{prefix}{body}{suffix}");
        let tree = Tree::new::<CppCode>(source.as_bytes());
        let root = tree.get_root();
        if root.has_error() {
            continue;
        }
        let Some(body) = root
            .children()
            .find(|n| {
                matches!(
                    Cpp::from(n.kind_id()),
                    Cpp::FunctionDefinition
                        | Cpp::FunctionDefinition2
                        | Cpp::FunctionDefinition3
                        | Cpp::FunctionDefinition4
                )
            })
            .and_then(|n| n.child_by_field_name("body"))
        else {
            continue;
        };
        let mut map = HashMap::from([(body.id(), nesting)]);
        let mut pending: Vec<_> = body.children().collect();
        pending.reverse();
        while let Some(node) = pending.pop() {
            CppCode::compute(&node, stats, &mut map);
            let mut children: Vec<_> = node.children().collect();
            children.reverse();
            pending.extend(children);
        }
        return;
    }
}
