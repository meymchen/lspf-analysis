# lspf-analysis

[English](./README.md) · [简体中文](./README.zh-CN.md)

**lspf-analysis** is a language server that reports code health for functions
and classes while you type.

It computes code metrics with a tree-sitter-based engine forked from
[rust-code-analysis](https://github.com/mozilla/rust-code-analysis), scores
them into a code health rating, and publishes the result as LSP diagnostics
through [lspf](https://github.com/meymchen/lspf).

Supported languages: **C++**, **Java**, **JavaScript**, **Python**, **Rust**,
**TypeScript**, **TSX**.

## What it reports

Hovering a function's name shows its measurements; a function below the
quality threshold gets a diagnostic under its name. The hover answers
on the name and nowhere else, so it adds to whatever the editor already says
about a symbol rather than displacing it. A client that would rather draw its
own list than read one function at a time asks for
[the per-function breakdown](#the-per-function-breakdown); the VS Code
extension puts it in an Explorer tree.

Where the engine computes class-level metrics — Java, today — a class or
interface is reported the same way, on its declaration line and on its name.

Quality is blended from four pillars. Each pillar takes the **worst** of its
metrics rather than their average, so a function cannot hide a bad number
behind a good one, and no property is counted twice.

| Pillar | Metric | Default threshold | Evidence |
| --- | --- | ---: | --- |
| **Control flow** | cognitive complexity | 15 | [Campbell 2018][c18], validated against measured understandability by [Muñoz Barón et al. 2020][mb20] |
| | cyclomatic complexity | 10 | [McCabe 1976][mc76]; not redundant with size at method level, [Landman et al. 2016][la16] |
| **Size** | statements (logical lines) | 30 | unit size in the SIG model, [Heitlager et al. 2007][hkv07], [SIG/TÜViT][sig] |
| **Vocabulary load** | names held at once | 8 | [Miller 1956][mi56], [Cowan 2001][co01] |
| | Halstead difficulty | 12 | vocabulary size is what loads working memory, [Peitek et al. 2021][pe21] |
| **Interface** | parameters | 4 | unit interfacing in the SIG model; the smell with the highest defect correlation in [Topuz 2022][to22] |

Class design has a separate scoring pillar. Its current thresholds come from
Java corpora, so only Java classes are scored. C++ class metrics are computed
and reported, but do not affect health scores until suitable thresholds are
established. Other languages without class metrics remain outside this pillar.

| Pillar | Metric | Default threshold | Evidence |
| --- | --- | ---: | --- |
| **Class design** | weighted methods per class | 34 | where "uncommon" starts in a benchmark of the 111 Java systems in the Qualitas.class Corpus, [Filó et al. 2015][fi15] |
| | public methods | 14 | the same catalogue's boundary for methods per class, read conservatively; [Ferreira et al. 2012][fe12] |
| | public attributes | 8 | the same, for fields per class |

### The formula

Each metric is scored on its own before anything is blended, so a function is
never rescued by being short if it is impenetrable. A raw value $r \ge 0$
with threshold $t > 0$ scores

```math
s(r) = \frac{100}{1 + \left(\dfrac{r}{t}\right)^{2}}
```

giving $s(0) = 100$, $s(t) = 50$, $s(2t) = 20$ and $s(3t) = 10$ — smooth,
monotone, and always in $(0, 100]$.

A pillar $P$ scores at its worst metric, with each metric evaluated against
its own threshold:

```math
s_P = \min_{m \in P} s(r_m)
```

and the pillars blend into quality as a weighted geometric mean:

```math
Q = \prod_{P} s_P^{w_P}
```

Here $w_P \ge 0$ are the normalized pillar weights, with $\sum_P w_P = 1$.
The default relative weights $(1, 1, 1, 0.5)$ become $(2/7, 2/7, 2/7, 1/7)$.

A geometric mean rather than an arithmetic one, so one collapsed pillar drags
the whole score down instead of hiding behind the others.

A class scores at its single pillar. A file blends its functions with its
classes the same geometric way, weighting each function by its length and
each class by how many methods it defines; with no scored classes the second
factor is absent and the file scores exactly what its functions do.

Quality maps to a band: **excellent** (≥80), **good** (≥50), **fair** (≥25),
**poor** (<25).

### Cognitive complexity coverage

Cognitive complexity adds one point to each function in a recursion cycle
resolved within the current file. Resolution uses unique lexical names;
Java calls additionally require a private, static, or final method with a
matching fixed argument count. Ambiguous bindings and dynamic receivers are
skipped.

The C++ implementation references [rust-code-analysis at 37e5d83](https://github.com/mozilla/rust-code-analysis/tree/37e5d83c056c8cbf827223d5814a93c5218df1a9).
C++ uses the unmodified `tree-sitter-cpp` 0.23.4 grammar. Functions,
constructors, destructors, conversion operators, lambdas, classes, structs,
unions, and namespaces have their own metric spaces. Standard `<cinttypes>`
format fragments such as `"%" PRIi32` are normalized at equal byte width for
parsing; reports retain the original source and positions.

C++ macro replacement lists contribute their source-visible control flow and
boolean sequences once, at the definition. Calls do not multiply that cost.
This is source analysis: it does not load headers, choose preprocessor branches,
or expand build-dependent macros. Recursion uses unique unqualified names in
the current file; overloads, qualified calls, and indirect calls are skipped.
The upstream C++ macro-complexity TODO is covered by replacement-list tests,
and the ignored format-macro regression (Mozilla issue 1142) runs here.
The upstream preprocessor warning/performance TODOs belong to Mozilla's
preprocessing pipeline, which this project does not use.

For Rust, unqualified `assert!`, `dbg!`, and `vec!` calls contribute the
complexity of their visible argument expressions, including nested macros
and the caller's nesting level. Macro expansion is not performed. Imported
or locally redefined macro names are skipped conservatively. Qualified and
cross-file calls, additional binding forms, and broader macro support are
tracked in [issue #1](https://github.com/meymchen/lspf-analysis/issues/1).

### C++ ABC and class metrics

C++ ABC counts explicit initialization, assignment, and increment/decrement
operations as A; call expressions, `new`, `delete`, and jumps into deeper
blocks as B; and comparisons, unary boolean tests, conditional expressions,
`else`, switch labels, and `catch` clauses as C. Grammar structure distinguishes
template brackets and declarators from comparisons and calls. Macro replacement
lists contribute once at their definition, without multiplying their bodies
at call sites. Implicit constructors, destructors, and overloaded operator calls
are not inferred from syntax.

NPA counts public data members and NPM counts public methods declared by each
class, struct, or union. Counts include static members, constructors,
destructors, conversion operators, and method templates. Access starts private
for `class` and public for `struct`/`union`, then follows access specifiers.
Function pointers are data members; friends and inherited members are excluded.
Anonymous union fields belong to their enclosing class. Nested classes keep
their own inventories. Preprocessor branches are visited in source order;
these counts describe the source file, not a selected build configuration.

WMC sums method cyclomatic complexity, excluding nested lambdas and local
classes. Declarations own the methods; an unambiguous matching class-external
definition supplies complexity without increasing NPM. Matching uses namespaces,
class names, parameter type tokens, and method cv/ref qualifiers, ignoring
parameter names and defaults. Generic template owners are matched where their
source signatures agree. Type aliases, semantic type equivalence, renamed
template parameters, and cross-file definitions are not resolved. Ambiguous
matches stay unresolved.

A method with a missing or malformed body leaves WMC incomplete. JSON then
reports `null` for the affected WMC sum, alongside `known_complexity` and
`unresolved_methods`; YAML/text use their non-finite value representation.
Explicitly defaulted, deleted, and pure declarations contribute zero executable
source complexity. Their compiler-generated behavior is outside this metric.
Both coverage fields also appear in the text metrics output. C++ ABC and class
metrics currently do not change health scores.

### What is deliberately left out

The engine computes more than the score uses. Each omission is a judgement
about evidence, not an oversight.

- **Maintainability index** — reported on each file as a familiar second
  opinion, never scored. Its constants are curve-fitted to one 1990s corpus,
  it averages away the distribution it summarises, and it is confounded by
  size ([Heitlager et al. 2007][hkv07], [van Deursen 2014][vd14],
  [El Emam et al. 2001][ee01], [Sjøberg et al. 2012][sj12]).
- **Comment density** — the replicated findings concern whether comments are
  *accurate*, not how many there are ([Rani et al. 2023][ra23]).
- **Number of exit points** — the single-exit rule is argued in style guides
  on both sides, but a literature search turns up no controlled study
  relating exit count to defects or comprehension.
- **Number of methods** — a count without the complexity weighting that
  makes WMC say something about how much a class does. It is carried on the
  class report as the weight a file balances its classes by, not scored.
- **ABC** — computed for Java and C++. It is reported but not scored: adding
  another size measure for only those languages would weaken comparability.
  It is hidden where it is not computed.

### Defaults

| Setting | Default | Meaning |
| --- | --- | --- |
| `complexityThreshold` | 15 | Cognitive complexity scoring 50% |
| `cyclomaticThreshold` | 10 | Cyclomatic complexity scoring 50% |
| `lengthThreshold` | 30 | Statements scoring 50% |
| `workingMemoryThreshold` | 8 | Names scoring 50% |
| `halsteadDifficultyThreshold` | 12 | Halstead difficulty scoring 50% |
| `parametersThreshold` | 4 | Parameters scoring 50% |
| `wmcThreshold` | 34 | Weighted methods per class scoring 50% |
| `publicMethodsThreshold` | 14 | Public methods per class scoring 50% |
| `publicAttributesThreshold` | 8 | Public attributes per class scoring 50% |
| `weights` | 1 / 1 / 1 / 0.5 | Relative pillar weights, normalized before use |
| `weights.classDesign` | 0.5 | How much a file's classes weigh against its functions |
| `qualityWarn` | 25 | Below this, a function is reported |
| `qualityError` | 10 | Below this, it is an error rather than a warning |

The three thresholds that predate the fourth pillar keep their names, so a
configuration written for the earlier model still means what it meant.

### References

- \[c18\] G. A. Campbell. *Cognitive Complexity: an overview and evaluation.*
  TechDebt 2018. <https://doi.org/10.1145/3194164.3194186>
- \[mb20\] M. Muñoz Barón, M. Wyrich, S. Wagner. *An Empirical Validation of
  Cognitive Complexity as a Measure of Source Code Understandability.* ESEM
  2020. <https://doi.org/10.1145/3382494.3410636> — see also L. Lavazza et
  al., JSS 197 (2023), <https://doi.org/10.1016/j.jss.2022.111561>, which is
  more sceptical that it improves on older measures.
- \[mc76\] T. J. McCabe. *A Complexity Measure.* IEEE TSE SE-2(4), 1976.
  <https://doi.org/10.1109/TSE.1976.233837>
- \[la16\] D. Landman, A. Serebrenik, E. Bouwers, J. J. Vinju. *Empirical
  analysis of the relationship between CC and SLOC in a large corpus of Java
  methods and C functions.* JSEP 28(7), 2016.
  <https://doi.org/10.1002/smr.1760>
- \[hkv07\] I. Heitlager, T. Kuipers, J. Visser. *A Practical Model for
  Measuring Maintainability.* QUATIC 2007.
  <https://doi.org/10.1109/QUATIC.2007.7>
- \[sig\] SIG/TÜV NORD CERT. *Evaluation Criteria Trusted Product
  Maintainability.* Thresholds calibrated per T. L. Alves, C. Ypma,
  J. Visser, *Deriving metric thresholds from benchmark data*, ICSM 2010.
  <https://doi.org/10.1109/ICSM.2010.5609747>
- \[mi56\] G. A. Miller. *The magical number seven, plus or minus two.*
  Psychological Review 63(2), 1956. <https://doi.org/10.1037/h0043158>
- \[co01\] N. Cowan. *The magical number 4 in short-term memory.* Behavioral
  and Brain Sciences 24(1), 2001.
  <https://doi.org/10.1017/S0140525X01003922>
- \[pe21\] N. Peitek, S. Apel, C. Parnin, A. Brechmann, J. Siegmund. *Program
  Comprehension and Code Complexity Metrics: An fMRI Study.* ICSE 2021.
  <https://doi.org/10.1109/ICSE43902.2021.00056>
- \[to22\] F. N. Topuz. *Empirical Evidence of the Consequences of Bad Smells in
  Software.* Auburn University, 2022.
  <https://etd.auburn.edu/handle/10415/8100>
- \[vd14\] A. van Deursen. *Think Twice Before Using the "Maintainability
  Index".* 2014.
  <https://avandeursen.com/2014/08/29/think-twice-before-using-the-maintainability-index/>
- \[ee01\] K. El Emam, S. Benlarbi, N. Goel, S. N. Rai. *The Confounding Effect
  of Class Size on the Validity of Object-Oriented Metrics.* IEEE TSE 27(7),
  2001. <https://doi.org/10.1109/32.935855>
- \[sj12\] D. I. K. Sjøberg, B. Anda, A. Mockus. *Questioning software
  maintenance metrics.* ESEM 2012.
  <https://doi.org/10.1145/2372251.2372269>
- \[ra23\] P. Rani, A. Blasi, N. Stulova, et al. *A decade of code comment
  quality assessment: a systematic literature review.* JSS 195, 2023.
  <https://doi.org/10.1016/j.jss.2022.111515>
- \[fi15\] T. Filó, M. Bigonha, K. Ferreira. *A Catalogue of Thresholds for
  Object-Oriented Software Metrics.* SOFTENG 2015.
  <https://personales.upv.es/thinkmind/dl/conferences/softeng/softeng_2015/softeng_2015_3_10_55070.pdf>
  — evaluated for bad-smell detection and fault prediction in the same
  authors' JBCS 2024 follow-up,
  <https://journals-sol.sbc.org.br/index.php/jbcs/article/view/3373>
- \[fe12\] K. Ferreira, M. Bigonha, R. Bigonha, L. Mendes, H. Almeida.
  *Identifying thresholds for object-oriented software metrics.* JSS 85(2),
  2012. <https://doi.org/10.1016/j.jss.2011.05.044>

[c18]: https://doi.org/10.1145/3194164.3194186
[mb20]: https://doi.org/10.1145/3382494.3410636
[mc76]: https://doi.org/10.1109/TSE.1976.233837
[la16]: https://doi.org/10.1002/smr.1760
[hkv07]: https://doi.org/10.1109/QUATIC.2007.7
[sig]: https://doi.org/10.1109/ICSM.2010.5609747
[mi56]: https://doi.org/10.1037/h0043158
[co01]: https://doi.org/10.1017/S0140525X01003922
[pe21]: https://doi.org/10.1109/ICSE43902.2021.00056
[to22]: https://etd.auburn.edu/handle/10415/8100
[vd14]: https://avandeursen.com/2014/08/29/think-twice-before-using-the-maintainability-index/
[ee01]: https://doi.org/10.1109/32.935855
[sj12]: https://doi.org/10.1145/2372251.2372269
[ra23]: https://doi.org/10.1016/j.jss.2022.111515
[fi15]: https://personales.upv.es/thinkmind/dl/conferences/softeng/softeng_2015/softeng_2015_3_10_55070.pdf
[fe12]: https://doi.org/10.1016/j.jss.2011.05.044

## Install

### VS Code

[`clients/vscode`](./clients/vscode) is the extension, and it carries the
server binary, so nothing else has to be installed. Until it is published,
build the VSIX for your machine:

```console
npm --prefix clients/vscode ci
npm --prefix clients/vscode run package
code --install-extension clients/vscode/lspf-analysis-<platform>-<version>.vsix
```

`npm run package -- --target darwin-arm64` builds for another platform;
[the extension's README](./clients/vscode/README.md) lists the supported ones.

### IntelliJ IDEA

[`clients/intellij`](./clients/intellij) is the plugin, built on the platform's
own LSP client, and it carries the server binary the same way. It needs
**2026.1.4 or later**: that release open-sourced the LSP client API, and before
it a plugin built on that API was inert outside the commercial IDEs. Until it is
published, build the ZIP and install it from disk:

```console
clients/intellij/gradlew -p clients/intellij buildPlugin
```

Then **Settings | Plugins | ⚙ | Install Plugin from Disk…** and pick
`clients/intellij/build/distributions/lspf-analysis-intellij-win32-x64-<version>.zip`.

Only Windows x64 ships with a binary today. Elsewhere the plugin works the same,
but install the server separately and point **Language server path** at it;
[the plugin's README](./clients/intellij/README.md) has the details.

### Visual Studio

[`clients/vs`](./clients/vs) contains a Windows x64 extension for Visual Studio
2026 and 2022. Open `lspf-analysis.sln` at the repository root and press F5 to
build and debug it in an experimental instance. Supported source editors start
analysis automatically. Diagnostics, hover tables, file health summaries, and
a sortable function tree use the same server as the other clients. Settings
include health thresholds and weights, diagnostics, server path, and protocol
tracing. See [the extension's README](./clients/vs/README.md) for packaging,
stdio/TCP/WebSocket debugging, and integration tests.

### The binary on its own

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

In VS Code and in IntelliJ IDEA the client does this for you. Any other editor
launches `lspf-analysis serve --stdio` the way it launches any language server,
for the language ids `java`, `rust`, `python`, `javascript`, `javascriptreact`,
`typescript` and `typescriptreact`.

Open a source file with a long or deeply nested function; its name should pick
up a warning, and hovering that name should report its four numbers.

### The file summary

A file's quality belongs to the whole document, so it has no honest range to
sit on. Instead, every analysis pushes a `lspfAnalysis/fileHealth`
notification:

```json
{
  "uri": "file:///src/lib.rs",
  "quality": 63.4,
  "grade": "good",
  "functions": 7,
  "below": 1,
  "bands": { "excellent": 4, "good": 2, "fair": 0, "poor": 1 },
  "worst": [
    {
      "name": "tangled",
      "quality": 22.1,
      "grade": "poor",
      "line": 84,
      "weakestPillar": "control flow",
      "weakestMetric": "cognitive complexity"
    }
  ]
}
```

The `bands` counts and the `worst` list — capped by `worstFunctions` — are
what the VS Code extension draws in its status bar hover. A client that does
not listen for the method ignores it, so nothing else has to change.

### The per-function breakdown

A client drawing its own UI needs the numbers rather than a rendering of
them, so it can ask for every function of one document with the
`lspfAnalysis/functionHealth` request:

```json
{ "uri": "file:///src/lib.rs" }
```

which answers with the functions in source order, each down to the measure
that set each pillar:

```json
{
  "uri": "file:///src/lib.rs",
  "functions": [
    {
      "name": "tangled",
      "startLine": 84,
      "endLine": 131,
      "quality": 22.1,
      "grade": "poor",
      "weakestPillar": "control flow",
      "weakestMetric": "cognitive complexity",
      "pillars": [
        {
          "name": "control flow",
          "score": 11.9,
          "measures": [
            {
              "name": "cognitive complexity",
              "value": 41,
              "threshold": 15,
              "score": 11.9
            }
          ]
        }
      ]
    }
  ]
}
```

The answer is `null` for a document the server has not analyzed. This is a
request rather than another field on `fileHealth` because that notification
is pushed on every keystroke, while the breakdown is an order of magnitude
more JSON that only a client with a view open has any use for. The VS Code
extension pulls it for its Code Health tree.

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
    },
    "locale": "zh-cn"
  }
}
```

- `diagnostics.perMetric` also reports a function that passes overall but
  has one badly scoring pillar, as an Information-level hint.
- `diagnostics.file` also reports the file as a whole when its quality falls
  below `qualityError`.

For a client that sends no configuration at all, `LSPF_ANALYSIS_SETTINGS` in
the environment takes the same JSON.

### Language

`locale` is the editor's language tag — `zh-cn`, `en`, whatever the client
has. Simplified Chinese and English are translated; anything else renders
English rather than a guess. The VS Code extension sends its own display
language, so nothing has to be set there.

It only decides text a person reads: hovers and diagnostic messages. What
travels in `lspfAnalysis/fileHealth` and `lspfAnalysis/functionHealth` — the
pillar and metric names, the grade words — stays English whatever the reader
sees, because a client keys off it. So does a diagnostic's `code` and
`source`. A client that draws its own UI translates that vocabulary itself,
as the VS Code extension does.

## Running from the command line

```console
lspf-analysis metrics --paths src
lspf-analysis metrics --paths src -O json --pr -o /tmp/report
lspf-analysis metrics --paths src -I '*.rs' -X '*/generated/*' -j 8
```

Without `--output-format` this prints the metrics tree per file plus each
file's quality; with one, it serializes `{metrics, health}` per file as
`json`, `toon`, or `cbor` — one for a program, one for a model, one for the
wire. Either way it ends with a repository summary: overall quality, the
count in each band, and the worst functions.

[`toon`](https://github.com/toon-format/toon) is for a report headed into a
model's context rather than a program's parser. A report is mostly rows that
say the same thing about different metrics, and TOON writes those keys once
as a header instead of on every row:

```toon
measures[2]{name,value,threshold,score}:
  cognitive complexity,10,15,69.23076923076923
  cyclomatic complexity,5,10,80
```

which on this repository's own sources comes out about a third smaller than
the same report as indented JSON. It has a single spelling, so `--pr` does
nothing to it.

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

The health module documents its formulas in LaTeX. rustdoc has no maths of
its own, so pass the KaTeX header to render them:

```console
RUSTDOCFLAGS="--html-in-header crates/lspf-analysis-core/katex.html" \
  cargo doc -p lspf-analysis-core --no-deps --open
```

docs.rs picks the same header up from `package.metadata.docs.rs`.

The VS Code client has its own tests:

```console
npm --prefix clients/vscode ci
npm --prefix clients/vscode test
```

Select one of three VS Code debug entries and press F5:

- **stdio** starts the Extension Development Host, which launches the server
  with `serve --stdio`, using the same transport as the distributed extension.
  It uses the local debug build unless `lspfAnalysis.server.path` is configured.
  Server logs appear in **Output → LSPF Analysis**.
- **tcp** starts the server under CodeLLDB on `127.0.0.1:9257` and an extension
  host that connects through TCP.
- **websocket** starts the server under CodeLLDB on `127.0.0.1:9258` and an
  extension host that connects through WebSocket.

Each entry builds the server and prepares the extension before launching.
TCP and WebSocket run the server and host as separate debug sessions, with
Rust and extension breakpoints available. The host retries while the server
starts. Stopping either session stops both; restart the entry after closing
or reloading the host. Server logs go to the integrated terminal. The stdio
entry supports extension breakpoints.

The menu shows only these three entries. The fixed server and host
configurations used by the network entries are hidden. No protocol prompt,
console listener, or Python helper is needed.

Run `npm --prefix clients/vscode run test:debug-transport` to build the server
and verify LSP initialization and shutdown over stdio, TCP, and WebSocket.
Tests that require native POSIX or Windows path behavior skip on other systems.

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
- [`clients/vscode`](./clients/vscode) — the VS Code extension, which packages
  the binary for one platform per VSIX.
- [`clients/intellij`](./clients/intellij) — the IntelliJ IDEA plugin, built on
  the platform's own LSP client and packaged the same way.
- [`clients/vs`](./clients/vs) — the Visual Studio 2026/2022 extension and its
  bundled language server.
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

```bibtex
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
