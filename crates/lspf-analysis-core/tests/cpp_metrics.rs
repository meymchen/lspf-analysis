use std::path::Path;

use lspf_analysis_core::{CppParser, FuncSpace, ParserTrait, metrics};

fn analyze(source: &str) -> FuncSpace {
    let path = Path::new("metrics.cpp");
    let parser = CppParser::new(source.as_bytes().to_vec(), path);
    metrics(&parser, path).unwrap()
}

fn find<'a>(space: &'a FuncSpace, name: &str) -> &'a FuncSpace {
    if space.name.as_deref() == Some(name) {
        return space;
    }
    for child in &space.spaces {
        if let Some(found) = search(child, name) {
            return found;
        }
    }
    panic!("missing {name} in {space:#?}");
}

fn search<'a>(space: &'a FuncSpace, name: &str) -> Option<&'a FuncSpace> {
    if space.name.as_deref() == Some(name) {
        Some(space)
    } else {
        space.spaces.iter().find_map(|child| search(child, name))
    }
}

#[test]
fn cpp_access_control_and_external_definition() {
    let space = analyze(
        "class C { int secret; public: int a, b; int (*callback)(int); void f(int x = 0); int *g() { return nullptr; } }; void C::f(int value) { if (value) return; }",
    );
    let c = find(&space, "C");
    assert_eq!(c.metrics.npa.class_na(), 4.);
    assert_eq!(c.metrics.npa.class_npa(), 3.);
    assert_eq!(c.metrics.npm.class_nm(), 2.);
    assert_eq!(c.metrics.npm.class_npm(), 2.);
    assert_eq!(c.metrics.wmc.class_wmc(), 3.);
    assert_eq!(space.metrics.wmc.total_wmc(), 3.);
}

#[test]
fn cpp_declarations_report_missing_complexity() {
    let space = analyze(
        "struct S { int x; private: int y; public: S() = default; ~S() {} virtual void abstract() = 0; void removed() = delete; void missing(); };",
    );
    let s = find(&space, "S");
    assert_eq!(s.metrics.npa.class_na(), 2.);
    assert_eq!(s.metrics.npa.class_npa(), 1.);
    assert_eq!(s.metrics.npm.class_nm(), 5.);
    let wmc = serde_json::to_value(&s.metrics.wmc).unwrap();
    assert!(wmc["classes"].is_null());
    assert_eq!(wmc["known_complexity"], 1.);
    assert_eq!(wmc["unresolved_methods"], 1);
}

#[test]
fn cpp_nested_classes_friends_and_lambdas_do_not_inflate_wmc() {
    let space = analyze(
        "struct Outer { int x; struct Inner { int y; void f() { if (y) return; } }; friend void free() { if (true) return; } void g() { auto f = [] { if (true) return; }; } };",
    );
    assert_eq!(find(&space, "Outer").metrics.npa.class_na(), 1.);
    assert_eq!(find(&space, "Outer").metrics.npm.class_nm(), 1.);
    assert_eq!(find(&space, "Outer").metrics.wmc.class_wmc(), 1.);
    assert_eq!(find(&space, "Inner").metrics.wmc.class_wmc(), 2.);
    assert_eq!(space.metrics.wmc.total_wmc(), 3.);
}

#[test]
fn cpp_overloads_cv_qualifiers_and_conversion_operators() {
    let space = analyze(
        "namespace n { struct C { int f(int); int f(double) const; operator bool() const; }; int C::f(int x) { return x; } int C::f(double value) const { if (value) return 1; return 0; } C::operator bool() const { return true; } }",
    );
    let c = find(&space, "C");
    assert_eq!(c.metrics.npm.class_nm(), 3.);
    assert_eq!(c.metrics.wmc.class_wmc(), 4.);
}

#[test]
fn cpp_template_methods_and_multiple_method_declarators() {
    let space = analyze(
        "template<class T> struct Box { T get(T); void f(), g(); }; template<class T> T Box<T>::get(T value) { return value; } template<class T> void Box<T>::f() {} template<class T> void Box<T>::g() {}",
    );
    let c = find(&space, "Box");
    assert_eq!(c.metrics.npm.class_nm(), 3.);
    assert_eq!(c.metrics.wmc.class_wmc(), 3.);
}

#[test]
fn cpp_abc_assignments_calls_and_conditions() {
    let space = analyze(
        "void f(int a, int b) { int x = 0; x += a; ++x; if (a && b > 0) call(); else x = 1; }",
    );
    let abc = &find(&space, "f").metrics.abc;
    assert_eq!(abc.assignments(), 4.);
    assert_eq!(abc.branches(), 1.);
    assert_eq!(abc.conditions(), 3.);
}

#[test]
fn cpp_abc_templates_are_not_comparisons_and_methods_are_not_calls() {
    let space = analyze(
        "template<class T> struct V {}; void f() { V<int> x; int (*callback)(int); auto p = new V<int>(); delete p; if (!(value > 1)) callback(0); }",
    );
    let abc = &find(&space, "f").metrics.abc;
    assert_eq!(abc.assignments(), 1.);
    assert_eq!(abc.branches(), 3.);
    assert_eq!(abc.conditions(), 1.);
}

#[test]
fn cpp_abc_macros_count_the_definition_once() {
    let space = analyze(
        "#define STEP(x) do { ++x; if (x) visit(x); } while (0)\nvoid f(int x) { STEP(x); STEP(x); }",
    );
    assert_eq!(space.metrics.abc.assignments_sum(), 1.);
    assert_eq!(space.metrics.abc.branches_sum(), 3.);
    assert_eq!(space.metrics.abc.conditions_sum(), 2.);
}

#[test]
fn cpp_abc_goto_only_counts_entry_into_deeper_blocks() {
    let space =
        analyze("void f() { goto inside; { inside: call(); goto outside; } outside: return; }");
    assert_eq!(find(&space, "f").metrics.abc.branches(), 2.);
}

#[test]
fn cpp_pure_methods_are_not_assignments_and_fields_are() {
    let space = analyze("struct S { int a = 1, b{2}; virtual void f() = 0; }; ");
    assert_eq!(find(&space, "S").metrics.abc.assignments(), 2.);
    assert_eq!(find(&space, "S").metrics.wmc.class_wmc(), 0.);
}

#[test]
fn cpp_member_template_overloads_are_counted_separately() {
    let space = analyze(
        "struct S { template<class T> void f() {} template<int N> void f() {} void g(void); }; void S::g() {}",
    );
    let s = find(&space, "S");
    assert_eq!(s.metrics.npm.class_nm(), 3.);
    assert_eq!(s.metrics.wmc.class_wmc(), 3.);
}

#[test]
fn cpp_metrics_do_not_enable_java_class_thresholds() {
    let space = analyze("class C { public: int a,b,c,d,e,f,g,h,i,j; void method() {} }; ");
    let report = lspf_analysis_core::health::file_health(
        &space,
        Path::new("metrics.cpp"),
        &lspf_analysis_core::health::HealthConfig::default(),
    );
    assert!(report.classes.is_empty());
    assert_eq!(find(&space, "C").metrics.npa.class_npa(), 10.);
}

#[test]
fn cpp_external_definitions_are_scoped_and_unmatched_types_stay_unknown() {
    let space = analyze(
        "namespace a { struct C { void f(int); }; } namespace b { struct C { void f(int); }; } void a::C::f(int value) { if (value) return; }",
    );
    assert_eq!(find(find(&space, "a"), "C").metrics.wmc.class_wmc(), 2.);
    assert!(
        find(find(&space, "b"), "C")
            .metrics
            .wmc
            .class_wmc()
            .is_nan()
    );
    let total = serde_json::to_value(&space.metrics.wmc).unwrap();
    assert_eq!(total["known_complexity"], 2.);
    assert_eq!(total["unresolved_methods"], 1);
    assert!(total["total"].is_null());
}

#[test]
fn cpp_anonymous_union_members_are_owned_by_the_enclosing_class() {
    let space = analyze("class C { union { int a; float b; }; public: union { int c; }; };");
    let c = find(&space, "C");
    assert_eq!(c.metrics.npa.class_na(), 3.);
    assert_eq!(c.metrics.npa.class_npa(), 1.);
    assert_eq!(space.metrics.npa.total_na(), 3.);
}

#[test]
fn cpp_conditional_members_and_defaulted_external_definitions() {
    let space = analyze(
        "struct S {\n#if FLAG\n int a;\n#else\n int b;\n#endif\n ~S();\n}; S::~S() = default;",
    );
    let s = find(&space, "S");
    assert_eq!(s.metrics.npa.class_npa(), 2.);
    assert_eq!(s.metrics.npm.class_npm(), 1.);
    assert_eq!(s.metrics.wmc.class_wmc(), 0.);
}

#[test]
fn cpp_conflicting_external_definitions_leave_wmc_unknown() {
    let space = analyze(
        "struct S { void f(); };\n#if FLAG\nvoid S::f() {}\n#else\nvoid S::f() { if (true) return; }\n#endif",
    );
    let s = find(&space, "S");
    assert_eq!(s.metrics.npm.class_nm(), 1.);
    assert!(s.metrics.wmc.class_wmc().is_nan());
}

#[test]
fn cpp_boolean_values_outside_control_statements_and_comments() {
    let space = analyze(
        "bool f(bool a, bool b, bool c) { if (!(/* note */ a > 0)) return true; return !a || (b && c); }",
    );
    assert_eq!(find(&space, "f").metrics.abc.conditions(), 4.);
}

#[test]
fn cpp_malformed_method_body_does_not_produce_complete_wmc() {
    let space = analyze("struct S { void f() { int x = ; } };");
    let s = find(&space, "S");
    assert_eq!(s.metrics.npm.class_nm(), 1.);
    assert!(s.metrics.wmc.class_wmc().is_nan());
    assert_eq!(s.metrics.wmc.unresolved_methods(), Some(1));
}
