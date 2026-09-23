# Rust conventions

Use the Rust version and components pinned in `rust-toolchain.toml`. Update the
pin deliberately, together with any formatting changes caused by the upgrade.

## Syntax node classification

These rules apply to handwritten code for every supported language. Generated
grammar enums remain generator-owned.

- Use `kind_id()` for node classification. Use a node's textual kind only when
  producing a name for display or diagnostics.
- Keep enum variants qualified, such as `Cpp::Comment`. Do not import grammar
  enum variants through glob imports or individual variant imports.
- For a single kind, compare integers explicitly:
  `node.kind_id() == Cpp::Comment as u16`. The generated comparison between a
  `u16` and an enum performs a conversion; it is not the direct integer check.
- Error nodes are an exception: use tree-sitter's `is_error()` predicate. The
  generated `Error` enum variant is a fallback value, not the error node's ID.
  `has_error()` also includes errors in descendants and is not equivalent.
- For multiple kinds, use `matches!(Cpp::from(node.kind_id()), ...)` or
  `match Cpp::from(node.kind_id())`. Prefer explicit `From` over an inferred
  `.into()` so the grammar is visible at the classification site.
- When checking the same node repeatedly, reuse a local enum value. Shared
  algorithms classify IDs within the appropriate language before sharing
  semantic categories. Numeric IDs are not interchangeable across grammars.
- Preserve all relevant IDs when a grammar has multiple symbols with the same
  name. Never replace a name-based check with only one of those variants.
- Store grammar-specific IDs in static lookup tables as qualified enum variants
  cast to `u16`. Do not use unexplained numeric literals or unchecked casts from
  integers to enums.

For example:

```rust
let kind = Cpp::from(node.kind_id());
match kind {
    Cpp::FunctionDefinition
    | Cpp::FunctionDefinition2
    | Cpp::FunctionDefinition3
    | Cpp::FunctionDefinition4 => handle_function(node),
    _ => {}
}
```

## Formatting and validation

Let rustfmt decide the wrapping and indentation of `|` patterns. Short patterns
may share a line; long patterns wrap. Do not enforce manual line breaks with
`rustfmt::skip`. Type prefixes and wrapping are readability choices and have no
runtime cost. Measure broader performance claims with benchmarks.

Before submitting Rust changes, run:

```console
cargo fmt --all -- --check
cargo fmt --manifest-path enums/Cargo.toml -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The local pre-commit hooks and the CI workflow run these checks. The audit
workflow runs `cargo deny check` against `deny.toml` when dependencies change.
