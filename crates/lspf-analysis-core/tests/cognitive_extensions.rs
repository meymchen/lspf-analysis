use std::path::Path;

use lspf_analysis_core::{FuncSpace, get_function_spaces, guess_language};

fn analyze(source: &str, extension: &str) -> FuncSpace {
    let path = format!("example.{extension}");
    let path = Path::new(&path);
    let language = guess_language(source.as_bytes(), path).0.unwrap();
    get_function_spaces(&language, source.as_bytes().to_vec(), path).unwrap()
}

fn score(space: &FuncSpace, name: &str) -> f64 {
    let mut pending = vec![space];
    while let Some(space) = pending.pop() {
        if space.name.as_deref() == Some(name) {
            return space.metrics.cognitive.cognitive();
        }
        pending.extend(&space.spaces);
    }
    panic!("function {name} not found");
}

#[test]
fn direct_recursion_is_counted_once_in_every_language() {
    for (extension, source) in [
        ("rs", "fn recur() { recur(); recur(); }"),
        ("py", "def recur():\n    recur()\n    recur()\n"),
        ("js", "function recur() { recur(); recur(); }"),
        ("ts", "function recur(): void { recur(); recur(); }"),
        ("tsx", "function recur(): void { recur(); recur(); }"),
        (
            "java",
            "class C { static void recur() { recur(); recur(); } }",
        ),
    ] {
        let space = analyze(source, extension);
        assert_eq!(score(&space, "recur"), 1.0, "{extension}");
        assert_eq!(space.metrics.cognitive.cognitive_sum(), 1.0, "{extension}");
        assert_eq!(space.metrics.cognitive.cognitive_max(), 1.0, "{extension}");
    }
}

#[test]
fn mutual_recursion_marks_only_cycle_members() {
    for (extension, source) in [
        ("rs", "fn a() { b(); } fn b() { a(); } fn caller() { a(); }"),
        (
            "py",
            "def a():\n    b()\ndef b():\n    a()\ndef caller():\n    a()\n",
        ),
        (
            "js",
            "function a() { b(); } function b() { a(); } function caller() { a(); }",
        ),
        (
            "ts",
            "function a() { b(); } function b() { a(); } function caller() { a(); }",
        ),
        (
            "tsx",
            "function a() { b(); } function b() { a(); } function caller() { a(); }",
        ),
        (
            "java",
            "class C { private void a() { b(); } private void b() { a(); } void caller() { a(); } }",
        ),
    ] {
        let space = analyze(source, extension);
        assert_eq!(score(&space, "a"), 1.0, "{extension}");
        assert_eq!(score(&space, "b"), 1.0, "{extension}");
        assert_eq!(score(&space, "caller"), 0.0, "{extension}");
        assert_eq!(space.metrics.cognitive.cognitive_sum(), 2.0, "{extension}");
    }
}

#[test]
fn shadowed_names_and_dynamic_receivers_are_not_recursion() {
    for (extension, source) in [
        ("rs", "fn recur(recur: fn()) { recur(); }"),
        ("rs", "fn recur() { let recur = || {}; recur(); }"),
        ("py", "def recur(recur):\n    recur()\n"),
        ("py", "def recur():\n    recur = other\n    recur()\n"),
        ("js", "function recur(recur) { recur(); }"),
        ("js", "function recur() { let recur = other; recur(); }"),
        ("ts", "function recur() { other.recur(); }"),
        ("java", "class C { void recur() { other.recur(); } }"),
        (
            "java",
            "class C { static void recur() { Runnable r = () -> recur(); } }",
        ),
        (
            "java",
            "class C extends Base { static void recur() { recur(1); } }",
        ),
        (
            "java",
            "class C { static void recur(Object... args) { recur(); } }",
        ),
        ("rs", "fn recur() { const recur: fn() = other; recur(); }"),
        (
            "js",
            "function recur() { eval('var recur = other'); recur(); }",
        ),
    ] {
        let space = analyze(source, extension);
        assert_eq!(score(&space, "recur"), 0.0, "{extension}: {source}");
    }
}

#[test]
fn nested_functions_have_their_own_call_edges() {
    let space = analyze("function outer() { function inner() { outer(); } }", "js");
    assert_eq!(score(&space, "outer"), 0.0);
    assert_eq!(score(&space, "inner"), 0.0);
    let space = analyze("fn outer() { fn inner() { inner(); } inner(); }", "rs");
    assert_eq!(score(&space, "outer"), 0.0);
    assert_eq!(score(&space, "inner"), 1.0);
}

#[test]
fn imports_and_independent_scopes_do_not_confuse_resolution() {
    for (extension, source, expected) in [
        ("rs", "use std::fmt; fn recur() { recur(); }", 1.0),
        ("py", "import os\ndef recur():\n    recur()\n", 1.0),
        (
            "js",
            "import other from 'other'; function recur() { recur(); }",
            1.0,
        ),
        (
            "js",
            "import recur from 'other'; function recur() { recur(); }",
            0.0,
        ),
        (
            "rs",
            "mod a { fn recur() { other::recur(); } } mod b { fn recur() {} }",
            0.0,
        ),
        (
            "rs",
            "struct C; impl C { fn recur() { recur(); } } fn recur() {}",
            0.0,
        ),
        (
            "js",
            "function recur() { { var recur = other; } recur(); }",
            0.0,
        ),
        (
            "js",
            "function recur() { const { recur } = other; recur(); }",
            0.0,
        ),
        (
            "java",
            "class C { static void recur() { recur(1); } static void recur(int x) {} }",
            0.0,
        ),
        (
            "py",
            "def recur():\n    pass\n@decorate\ndef recur():\n    recur()\n",
            0.0,
        ),
    ] {
        let space = analyze(source, extension);
        assert_eq!(
            space.metrics.cognitive.cognitive_sum(),
            expected,
            "{extension}: {source}"
        );
    }
}

#[test]
fn recursion_reaches_the_health_report() {
    use lspf_analysis_core::health::{HealthConfig, file_health};
    let space = analyze("fn recur() { recur(); }", "rs");
    let health = file_health(&space, Path::new("example.rs"), &HealthConfig::default());
    let function = health
        .functions
        .iter()
        .find(|function| function.name.as_deref() == Some("recur"))
        .unwrap();
    let measure = function
        .scores
        .control_flow
        .measures
        .iter()
        .find(|measure| measure.name == "cognitive complexity")
        .unwrap();
    assert_eq!(measure.value, 1.0);
}

#[test]
fn rust_macro_arguments_contribute_visible_complexity() {
    for (source, expected) in [
        ("fn f() { assert!(a && b); }", 1.0),
        ("fn f() { dbg!(if a { 1 } else { 2 }); }", 2.0),
        ("fn f() { vec![if a { 1 } else { 2 }; 3]; }", 2.0),
        ("fn f() { dbg!(a && b, c && d); }", 2.0),
        ("fn f() { assert!(true, \"if while &&\"); }", 0.0),
        ("fn f() { if a { dbg!(if b { 1 } else { 2 }); } }", 4.0),
        ("fn f() { dbg!(dbg!(a && b)); }", 1.0),
        (
            "fn f() { vec![if a { 1 } else { 2 }, if b { 3 } else { 4 }]; }",
            4.0,
        ),
        (
            "fn f() { dbg!(call::<A, B>(a && b), r#\"if && while\"#); }",
            1.0,
        ),
        ("fn f() { dbg!({ while a { if b { break; } } }); }", 3.0),
        ("fn f() { dbg!(a && b); dbg!(c && d); }", 2.0),
        ("fn f() { custom!(if a { b() }); }", 0.0),
        ("fn f() { dbg!(if); }", 0.0),
        ("use custom::dbg; fn f() { dbg!(a && b); }", 0.0),
        ("use custom::*; fn f() { dbg!(a && b); }", 0.0),
        (
            "#[macro_use] extern crate custom; fn f() { dbg!(a && b); }",
            0.0,
        ),
        (
            "macro_rules! dbg { ($($t:tt)*) => {} } fn f() { dbg!(a && b); }",
            0.0,
        ),
    ] {
        let space = analyze(source, "rs");
        assert_eq!(score(&space, "f"), expected, "{source}");
        assert_eq!(
            space.metrics.cognitive.cognitive_sum(),
            expected,
            "{source}"
        );
    }
}
