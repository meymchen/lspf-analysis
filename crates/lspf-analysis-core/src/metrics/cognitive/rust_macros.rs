//! Count source-visible expressions in a small, explicit set of standard macros.
//! Expansion and source mapping: https://github.com/meymchen/lspf-analysis/issues/1.

use std::collections::{HashMap, HashSet};

use crate::node::Tree;
use crate::{Node, Rust, RustCode};

use super::{Cognitive, Stats};

const SUPPORTED: &[&str] = &["assert", "dbg", "vec"];
const MAX_DEPTH: usize = 32;

#[derive(Default)]
pub(super) struct Macros {
    shadowed: HashSet<String>,
    wildcard_import: bool,
}

impl Macros {
    pub(super) fn new(root: Node, code: &[u8]) -> Self {
        let mut result = Self::default();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if node.kind_id() == Rust::MacroDefinition as u16 {
                if let Some(name) = node.child_by_field_name("name") {
                    result.shadowed.insert(
                        String::from_utf8_lossy(&code[name.start_byte()..name.end_byte()])
                            .into_owned(),
                    );
                }
                continue;
            }
            if node.kind_id() == Rust::UseDeclaration as u16 {
                let import = String::from_utf8_lossy(&code[node.start_byte()..node.end_byte()]);
                result.wildcard_import |= import.contains('*');
                for word in import.split(|c: char| !c.is_alphanumeric() && c != '_') {
                    if SUPPORTED.contains(&word) {
                        result.shadowed.insert(word.to_owned());
                    }
                }
            }
            if matches!(
                Rust::from(node.kind_id()),
                Rust::AttributeItem | Rust::InnerAttributeItem
            ) && String::from_utf8_lossy(&code[node.start_byte()..node.end_byte()])
                .contains("macro_use")
            {
                result.wildcard_import = true;
            }
            pending.extend(node.children());
        }
        result
    }

    pub(super) fn compute(
        &self,
        node: Node,
        code: &[u8],
        stats: &mut Stats,
        nesting: (usize, usize, usize),
    ) {
        self.compute_at_depth(node, code, stats, nesting, 0);
    }

    fn compute_at_depth(
        &self,
        node: Node,
        code: &[u8],
        stats: &mut Stats,
        nesting: (usize, usize, usize),
        depth: usize,
    ) {
        if depth >= MAX_DEPTH || node.has_error() {
            return;
        }
        let Some(name) = node.child_by_field_name("macro") else {
            return;
        };
        let Ok(name) = std::str::from_utf8(&code[name.start_byte()..name.end_byte()]) else {
            return;
        };
        // Qualified paths, imports and local definitions need macro name resolution.
        if !SUPPORTED.contains(&name) || self.shadowed.contains(name) || self.wildcard_import {
            return;
        }
        let Some(tokens) = node.children().find(|child| {
            matches!(
                Rust::from(child.kind_id()),
                Rust::TokenTree | Rust::TokenTree2
            )
        }) else {
            return;
        };
        if tokens.end_byte() <= tokens.start_byte() + 1 {
            return;
        }
        let content = &code[tokens.start_byte() + 1..tokens.end_byte() - 1];
        let (prefix, suffix) = if name == "vec" {
            (b"fn __macro_args() { [".as_slice(), b"]; }".as_slice())
        } else {
            (
                b"fn __macro_args() { __args(".as_slice(),
                b"); }".as_slice(),
            )
        };
        let source = [prefix, content, suffix].concat();
        let tree = Tree::new::<RustCode>(&source);
        let root = tree.get_root();
        if root.has_error() {
            return;
        }
        let Some(function) = root
            .children()
            .find(|child| child.kind_id() == Rust::FunctionItem as u16)
        else {
            return;
        };
        let Some(body) = function.child_by_field_name("body") else {
            return;
        };
        let Some(statement) = body
            .children()
            .find(|child| child.kind_id() == Rust::ExpressionStatement as u16)
        else {
            return;
        };
        let Some(expression) = statement.children().find(|child| child.is_named()) else {
            return;
        };
        let arguments = if name == "vec" {
            Some(expression)
        } else {
            expression.child_by_field_name("arguments")
        };
        let Some(arguments) = arguments else {
            return;
        };
        let expressions: Vec<_> = arguments
            .children()
            .filter(|child| {
                child.is_named()
                    && !matches!(
                        Rust::from(child.kind_id()),
                        Rust::LineComment | Rust::BlockComment
                    )
            })
            .collect();
        if name == "assert" && expressions.is_empty() {
            return;
        }
        for expression in expressions {
            // Each argument is a separate logical sequence. Synthetic wrapper nodes
            // must not reset the caller's nesting or create additional function spaces.
            stats.boolean_seq.reset();
            let mut map = HashMap::new();
            if let Some(parent) = expression.parent() {
                map.insert(parent.id(), nesting);
            }
            let mut pending = vec![expression];
            while let Some(node) = pending.pop() {
                // Function items belong to their own metric space, not the caller.
                if node.kind_id() == Rust::FunctionItem as u16 {
                    continue;
                }
                RustCode::compute(&node, stats, &mut map);
                if node.kind_id() == Rust::MacroInvocation as u16 {
                    self.compute_at_depth(node, &source, stats, map[&node.id()], depth + 1);
                    continue;
                }
                let children: Vec<_> = node.children().collect();
                pending.extend(children.into_iter().rev());
            }
        }
        stats.boolean_seq.reset();
    }
}
