use tree_sitter::Language;

mk_langs!(
    // 1) Name for enum
    // 2) tree-sitter function to call to get a Language
    (Java, tree_sitter_java),
    (Rust, tree_sitter_rust),
    (Python, tree_sitter_python),
    (Tsx, tree_sitter_tsx),
    (Typescript, tree_sitter_typescript),
    (Javascript, tree_sitter_javascript)
);
