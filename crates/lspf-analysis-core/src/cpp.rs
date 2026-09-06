//! Declarator traversal for the unmodified tree-sitter-cpp grammar.
use crate::{Cpp, Node};

mod classes;
pub(crate) use classes::{compute as compute_class_metrics, method_declarator};

/// Standard <cinttypes> string fragments are opaque to tree-sitter. Replace
/// only fragments adjacent to a string with equal-width quoted placeholders;
/// the original source remains the authority for names, metrics and locations.
pub(crate) fn normalize_format_macros(root: tree_sitter::Node, code: &[u8]) -> Option<Vec<u8>> {
    fn format_macro(text: &[u8]) -> bool {
        let Some(rest) = text
            .strip_prefix(b"PRI")
            .or_else(|| text.strip_prefix(b"SCN"))
        else {
            return false;
        };
        if !rest.first().is_some_and(|c| b"diouxX".contains(c)) {
            return false;
        }
        let size = &rest[1..];
        let size = size
            .strip_prefix(b"LEAST")
            .or_else(|| size.strip_prefix(b"FAST"))
            .unwrap_or(size);
        matches!(size, b"8" | b"16" | b"32" | b"64" | b"MAX" | b"PTR")
    }
    fn ends_in_string(node: tree_sitter::Node) -> bool {
        if matches!(
            Cpp::from(node.kind_id()),
            Cpp::StringLiteral | Cpp::RawStringLiteral | Cpp::ConcatenatedString
        ) {
            return true;
        }
        node.is_error()
            && node
                .named_children(&mut node.walk())
                .last()
                .is_some_and(ends_in_string)
    }
    let mut normalized = None;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.kind_id() == Cpp::Identifier as u16 && format_macro(&code[node.byte_range()]) {
            let mut previous = node.prev_named_sibling();
            while previous.is_some_and(|n| n.kind_id() == Cpp::Comment as u16) {
                previous = previous.and_then(|n| n.prev_named_sibling());
            }
            if previous.is_some_and(ends_in_string) {
                let bytes = normalized.get_or_insert_with(|| code.to_vec());
                bytes[node.byte_range()].fill(b' ');
                bytes[node.start_byte()] = b'"';
                bytes[node.end_byte() - 1] = b'"';
            }
        }
        pending.extend(node.children(&mut node.walk()));
    }
    normalized
}

pub(crate) fn declarator(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("declarator").or_else(|| {
        node.children().find(|n| {
            n.is_named()
                && matches!(
                    Cpp::from(n.kind_id()),
                    Cpp::Declarator
                        | Cpp::FieldDeclarator
                        | Cpp::TypeDeclarator
                        | Cpp::AbstractDeclarator
                        | Cpp::ParenthesizedDeclarator
                        | Cpp::ParenthesizedDeclarator2
                        | Cpp::ParenthesizedDeclarator3
                        | Cpp::AbstractParenthesizedDeclarator
                        | Cpp::AttributedDeclarator
                        | Cpp::AttributedDeclarator2
                        | Cpp::AttributedDeclarator3
                        | Cpp::PointerDeclarator
                        | Cpp::PointerDeclarator2
                        | Cpp::PointerTypeDeclarator
                        | Cpp::AbstractPointerDeclarator
                        | Cpp::FunctionDeclarator
                        | Cpp::FunctionDeclarator2
                        | Cpp::FunctionDeclarator3
                        | Cpp::AbstractFunctionDeclarator
                        | Cpp::ArrayDeclarator
                        | Cpp::ArrayDeclarator2
                        | Cpp::ArrayDeclarator3
                        | Cpp::AbstractArrayDeclarator
                        | Cpp::InitDeclarator
                        | Cpp::VariadicDeclarator
                        | Cpp::ReferenceDeclarator
                        | Cpp::ReferenceDeclarator2
                        | Cpp::ReferenceDeclarator3
                        | Cpp::ReferenceDeclarator4
                        | Cpp::AbstractReferenceDeclarator
                        | Cpp::StructuredBindingDeclarator
                        | Cpp::NewDeclarator
                )
        })
    })
}

pub(crate) fn function_declarator(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = declarator(node)?;
    let mut result = None;
    loop {
        if matches!(
            Cpp::from(current.kind_id()),
            Cpp::FunctionDeclarator
                | Cpp::FunctionDeclarator2
                | Cpp::FunctionDeclarator3
                | Cpp::AbstractFunctionDeclarator
        ) {
            result = Some(current);
        }
        let Some(next) = declarator(current) else {
            break;
        };
        current = next;
    }
    result
}

pub(crate) fn function_name(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = declarator(node)?;
    loop {
        if matches!(
            Cpp::from(current.kind_id()),
            Cpp::Identifier
                | Cpp::FieldIdentifier
                | Cpp::TypeIdentifier
                | Cpp::QualifiedIdentifier
                | Cpp::QualifiedIdentifier2
                | Cpp::QualifiedIdentifier3
                | Cpp::QualifiedIdentifier4
                | Cpp::DestructorName
                | Cpp::OperatorName
                | Cpp::OperatorCast
                | Cpp::TemplateFunction
                | Cpp::TemplateMethod
        ) {
            return Some(current);
        }
        current = declarator(current)?;
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::path::Path;

    fn analyze(source: &str) -> FuncSpace {
        let path = Path::new("test.cpp");
        let parser = CppParser::new(source.as_bytes().to_vec(), path);
        assert!(
            !parser.get_root().has_error(),
            "{}",
            parser.get_root().0.to_sexp()
        );
        metrics(&parser, path).unwrap()
    }

    #[test]
    fn cpp_declarators_and_parameters() {
        let space = analyze(
            "int *pointer(int x, char y) { return nullptr; }\nint zero(void) { return 0; }\nint unnamed(char, bool, int) { return 0; }\nint (*factory(int n))(double) { return nullptr; }",
        );
        let actual: Vec<_> = space
            .spaces
            .iter()
            .map(|s| (s.name.as_deref(), s.metrics.nargs.fn_args_sum()))
            .collect();
        assert_eq!(
            actual,
            vec![
                (Some("pointer"), 2.),
                (Some("zero"), 0.),
                (Some("unnamed"), 3.),
                (Some("factory"), 1.)
            ]
        );
    }

    #[test]
    fn cpp_spaces_and_lambdas() {
        let space = analyze(
            "namespace ns { struct S { S() {} ~S() {} operator bool() const { return true; } int operator()(int x) { auto f = [](int a, int b) { if (a) return b; return 0; }; return f(x, x); } }; }",
        );
        let ns = &space.spaces[0];
        assert_eq!(ns.kind, SpaceKind::Namespace);
        let class = &ns.spaces[0];
        assert_eq!(class.kind, SpaceKind::Struct);
        assert_eq!(class.spaces.len(), 4);
        assert_eq!(class.spaces[0].name.as_deref(), Some("S"));
        assert_eq!(class.spaces[1].name.as_deref(), Some("~S"));
        assert!(
            class.spaces[2]
                .name
                .as_deref()
                .unwrap()
                .starts_with("operator bool")
        );
        let method = &class.spaces[3];
        assert_eq!(method.name.as_deref(), Some("operator()"));
        assert_eq!(method.spaces.len(), 1);
        assert_eq!(method.spaces[0].metrics.nargs.closure_args_sum(), 2.);
        assert_eq!(space.metrics.nom.functions_sum(), 4.);
        assert_eq!(space.metrics.nom.closures_sum(), 1.);
    }

    #[test]
    fn cpp_complexity() {
        let space = analyze(
            "int f(int a, int b, int c, int d) { if (a && b || c && d) return 1; else if (a) return 2; else return 0; }",
        );
        let m = &space.spaces[0].metrics;
        assert_eq!(m.cognitive.cognitive_sum(), 6.);
        assert_eq!(m.cyclomatic.cyclomatic_sum(), 6.);
        assert_eq!(m.nexits.exit_sum(), 3.);
        assert!(m.halstead.operators() > 0.);
        assert!(m.halstead.operands() > 0.);
        assert!(m.working_memory.wm() > 0.);
    }

    #[test]
    fn cpp_range_loop_and_ternary() {
        let space =
            analyze("int f() { for (auto x : values) { if (x) return x ? 1 : 2; } return 0; }");
        assert_eq!(space.spaces[0].metrics.cognitive.cognitive_sum(), 6.);
        assert_eq!(space.spaces[0].metrics.cyclomatic.cyclomatic_sum(), 4.);
    }

    #[test]
    fn cpp_recursion_and_shadowing() {
        let space = analyze(
            "int f(int n) { return n ? f(n - 1) : 0; }\nint g(int n) { return h(n); }\nint h(int n) { return g(n); }",
        );
        assert_eq!(space.spaces[0].metrics.cognitive.cognitive_sum(), 2.);
        assert_eq!(space.spaces[1].metrics.cognitive.cognitive_sum(), 1.);
        assert_eq!(space.spaces[2].metrics.cognitive.cognitive_sum(), 1.);
        let shadow = analyze("int f(int (*f)(int)) { return f(1); }");
        assert_eq!(shadow.spaces[0].metrics.cognitive.cognitive_sum(), 0.);
    }

    #[test]
    fn cpp_macro_replacement_lists() {
        let space = analyze(
            "#define CHECK(x) do { \\\n if (x) use(x); \\\n} while (0)\n#define BOTH(a,b) ((a) && (b))\nvoid f() { CHECK(true); CHECK(false); }\n",
        );
        assert_eq!(space.metrics.cognitive.cognitive_sum(), 4.);
        assert_eq!(space.spaces[0].metrics.cognitive.cognitive_sum(), 0.);
    }

    #[test]
    fn cpp_upstream_format_macro_regression() {
        // Upstream c_langs_macros::test_fn_id_strings was ignored (issue 1142).
        analyze("void f() { nsPrintfCString(\"%\" PRIi32, lifetime.mTag); }");
    }

    #[test]
    fn cpp_comments_and_lines() {
        let space = analyze("// comment\nint f() {\n  int x = 0; // trailing\n  return x;\n}\n");
        assert_eq!(space.metrics.loc.cloc(), 2.);
        assert_eq!(space.metrics.loc.lloc(), 2.);
        assert_eq!(space.metrics.loc.ploc(), 4.);
    }

    #[test]
    fn cpp_default_variadic_and_template_parameters() {
        let space = analyze(
            "template<class T> int f(T x, int y = 1, ...) { return y; }\nvoid g() { auto a = [] { return 1; }; auto b = [](auto... xs) { return 2; }; }",
        );
        assert_eq!(space.spaces[0].metrics.nargs.fn_args_sum(), 3.);
        assert_eq!(
            space.spaces[1].spaces[0].metrics.nargs.closure_args_sum(),
            0.
        );
        assert_eq!(
            space.spaces[1].spaces[1].metrics.nargs.closure_args_sum(),
            1.
        );
    }

    #[test]
    fn cpp_format_macro_offsets_and_unrelated_identifiers() {
        let source = "void f() { print(\"%\" /* format */ PRId64, x); scan(\"%\" SCNu32, &x); }\nint g() { int PRIi32 = 1; return PRIi32; }";
        let parser = CppParser::new(source.as_bytes().to_vec(), Path::new("f.cpp"));
        assert!(
            !parser.get_root().has_error(),
            "{}",
            parser.get_root().0.to_sexp()
        );
        assert_eq!(parser.get_code(), source.as_bytes());
        let space = metrics(&parser, Path::new("f.cpp")).unwrap();
        assert_eq!(space.spaces[1].name.as_deref(), Some("g"));
        assert_eq!(space.spaces[1].start_line, 2);
        let mut pending = vec![parser.get_root()];
        let mut macro_identifiers = 0;
        while let Some(node) = pending.pop() {
            if node.kind_id() == Cpp::Identifier as u16
                && &source.as_bytes()[node.start_byte()..node.end_byte()] == b"PRIi32"
            {
                macro_identifiers += 1;
            }
            pending.extend(node.children());
        }
        assert_eq!(macro_identifiers, 2);
    }

    #[test]
    fn cpp_alternative_boolean_operators() {
        let space = analyze("bool f(bool a, bool b, bool c) { return a and b or c; }");
        assert_eq!(space.spaces[0].metrics.cognitive.cognitive_sum(), 2.);
        assert_eq!(space.spaces[0].metrics.cyclomatic.cyclomatic_sum(), 3.);
    }

    #[test]
    fn cpp_overloads_and_scopes_are_not_false_recursion() {
        for source in [
            "int f(int x) { return f(1.0); } double f(double x) { return x; }",
            "namespace a { int f(int x) { return x; } } namespace b { int f(int x) { return a::f(x); } }",
            "int f(int x) { int (*f)(int) = other; return f(x); }",
        ] {
            assert_eq!(analyze(source).metrics.cognitive.cognitive_sum(), 0.);
        }
    }
}
