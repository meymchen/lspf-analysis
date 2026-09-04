# lspf-analysis

**lspf-analysis** is a language server that tells you, while you type, which
functions are getting hard to work with.

It computes code metrics with a tree-sitter-based engine forked from
[rust-code-analysis](https://github.com/mozilla/rust-code-analysis), scores
them into a code health rating, and publishes the result as LSP diagnostics
through [lspf](https://github.com/meymchen/lspf).

Supported languages: **JavaScript**, **Python**, **Rust**, **TypeScript**,
**TSX**.

## What it reports

Hovering a function shows four numbers; a function below the quality
threshold gets a diagnostic on its signature line.

| Metric | What it measures | Where it comes from |
| --- | --- | --- |
| **Complexity** | How tangled the control flow is | Cognitive complexity (Campbell) |
| **Method length** | How much a function does | Logical lines of code, i.e. statements |
| **Working memory** | Names you have to hold at once | The busiest statement in the function |
| **Quality** | The three, blended | See below |

### The formula

Each pillar is scored on its own first, so a function is never rescued by
being short if it is impenetrable. A raw value `r >= 0` with threshold
`t > 0` scores

```text
s(r) = 100 / (1 + (r/t)²)
```

giving `s(0) = 100`, `s(t) = 50`, `s(2t) = 20`, `s(3t) = 10` — smooth,
monotone, and always in `(0, 100]`.

The three sub-scores blend into quality as a weighted geometric mean:

```text
quality = s_complexity^w₁ · s_length^w₂ · s_working_memory^w₃    (Σw = 1)
```

A geometric mean rather than an arithmetic one, so one collapsed pillar
drags the whole score down instead of hiding behind the other two.

Quality maps to a band: **excellent** (≥80), **good** (≥50), **fair** (≥25),
**poor** (<25).

### Defaults

| Setting | Default | Meaning |
| --- | --- | --- |
| `complexityThreshold` | 15 | Cognitive complexity scoring 50% |
| `lengthThreshold` | 30 | Statements scoring 50% |
| `workingMemoryThreshold` | 8 | Names scoring 50% (working memory holds 5–9) |
| `weights` | 1 / 1 / 1 | Relative pillar weights, normalized before use |
| `qualityWarn` | 25 | Below this, a function is reported |
| `qualityError` | 10 | Below this, it is an error rather than a warning |

## Install

```console
cargo install --path crates/lspf-analysis
```

This puts a `lspf-analysis` binary in Cargo's bin directory (`~/.cargo/bin`
by default). Make sure that directory is on your `PATH`.

## Running the server

```console
lspf-analysis serve                       # stdio: what editors launch
lspf-analysis serve --stdio               # the same, said explicitly
lspf-analysis serve --tcp 127.0.0.1:9257  # one TCP client
lspf-analysis serve --ws 127.0.0.1:9258   # one WebSocket client
```

The three transports are mutually exclusive. Logs go to stderr; stdout
carries the LSP wire protocol and nothing else. Set `RUST_LOG=lspf=trace` to
see the connection come up.

### Editor setup

VS Code has no built-in generic LSP client, so install a thin one such as
[Generic LSP Client (v2)](https://marketplace.visualstudio.com/items?itemName=zsol.vscode-glspc),
then add to `settings.json`:

```json
{
  "glspc.server.command": "lspf-analysis",
  "glspc.server.commandArguments": ["serve", "--stdio"],
  "glspc.server.languageId": ["rust", "python", "javascript", "typescript"]
}
```

Open a source file with a long or deeply nested function; the signature line
should pick up a warning, and hovering the function name should report its
four numbers.

### Configuration

Settings live under the `lspfAnalysis` section, in `initializationOptions` or
in `workspace/didChangeConfiguration`. Anything omitted keeps its default,
and changing them republishes every open document:

```json
{
  "lspfAnalysis": {
    "health": {
      "complexityThreshold": 20,
      "qualityWarn": 40
    },
    "diagnostics": {
      "enabled": true,
      "perMetric": false,
      "file": false
    }
  }
}
```

- `diagnostics.perMetric` also reports a function that passes overall but
  has one badly scoring pillar, as an Information-level hint.
- `diagnostics.file` also reports the file as a whole when its quality falls
  below `qualityError`.

For a client that sends no configuration at all, `LSPF_ANALYSIS_SETTINGS` in
the environment takes the same JSON.

## Running from the command line

```console
lspf-analysis metrics --paths src
lspf-analysis metrics --paths src -O json --pr -o /tmp/report
lspf-analysis metrics --paths src -I '*.rs' -X '*/generated/*' -j 8
```

Without `--output-format` this prints the metrics tree per file plus each
file's quality; with one, it serializes `{metrics, health}` per file as
`json`, `yaml`, `toml`, or `cbor`. Either way it ends with a repository
summary: overall quality, the count in each band, and the worst functions.

## Building and testing

```console
cargo build --workspace
cargo test --workspace
```

The metrics and health scores of one small file per language are covered by
snapshot tests in `crates/lspf-analysis-core/tests`. To review a snapshot
change, install [cargo-insta](https://crates.io/crates/cargo-insta) and run:

```console
cargo insta test --review
```

The protocol behavior — diagnostics on open, edit and close, configuration
changes, and hover — is covered end to end in
`crates/lspf-analysis/tests/server_journey.rs`, over lspf's in-memory
transport.

### Updating grammars

Bump the grammar crate versions in `crates/lspf-analysis-core/Cargo.toml` and
`enums/Cargo.toml`, then run `./recreate-grammars.sh` to regenerate the
language bindings in `crates/lspf-analysis-core/src/languages`. Use
`check-grammar-crate.py` to diff the metrics computed on a large repository
before and after the bump.

## Layout

- [`crates/lspf-analysis-core`](./crates/lspf-analysis-core) — the metrics
  engine and the health scoring layer.
- [`crates/lspf-analysis`](./crates/lspf-analysis) — the language server and
  its `lspf-analysis` binary.
- [`enums`](./enums) — the generator for the tree-sitter node kind bindings.

## Credits

The metrics engine is derived from
[**rust-code-analysis**](https://github.com/mozilla/rust-code-analysis) by
Mozilla, and this project would not exist without it. Its authors built the
tree-sitter parsing layer, the language abstractions, the space model and
nearly every metric computed here; what this project adds on top is a
thin scoring layer and a language server. The upstream repository is no
longer maintained, which is the reason for the fork — the work itself
remains excellent, and all credit for it belongs to the original authors.

If you use the metrics in academic work, cite their paper:

```
@article{ARDITO2020100635,
    title = {rust-code-analysis: A Rust library to analyze and extract maintainability information from source codes},
    journal = {SoftwareX},
    volume = {12},
    pages = {100635},
    year = {2020},
    issn = {2352-7110},
    doi = {https://doi.org/10.1016/j.softx.2020.100635},
    url = {https://www.sciencedirect.com/science/article/pii/S2352711020303484},
    author = {Luca Ardito and Luca Barbato and Marco Castelluccio and Riccardo Coppola and Calixte Denizet and Sylvestre Ledru and Michele Valsesia},
    keywords = {Algorithm, Software metrics, Software maintainability, Software quality},
}
```

## Licenses

- Mozilla-defined grammars are released under the MIT license.
- **lspf-analysis** and **lspf-analysis-core** are released under the
  [Mozilla Public License v2.0](https://www.mozilla.org/MPL/2.0/), inherited
  from rust-code-analysis.
