# LSPF Analysis for VS Code

[English][readme-en] · [简体中文][readme-zh]

Find complex functions and inspect the code health of your active file while
you edit. LSPF Analysis shows measurements on hover, reports low scores in
Problems, and lists functions by quality in the Explorer.

Supported languages: **C++**, **Java**, **JavaScript**, **Python**, **Rust**,
**TypeScript**, and **TSX**.

The extension includes the [lspf-analysis][project] language server. You do
not need to install Rust or configure another LSP client to use it. The
interface, hovers, and diagnostics support English and Simplified Chinese,
following VS Code's display language.

## Getting started

1. Install the extension in VS Code 1.106.0 or later.
2. Open a supported source file. Analysis starts automatically.
3. Hover over a function name to inspect its measurements and score.
4. Expand **Code Health** in the Explorer to see the active file's functions,
   ordered by lowest score first. Click a function to jump to its definition.

Functions below the warning threshold also appear in **Problems**. Hover over
the status bar's code health item for a file summary and links to the
functions with the lowest scores.

## What you can inspect

- Function hovers show measurements and the metrics behind each score.
- The **Code Health** tree groups measurements into scoring pillars. Its
  toolbar switches between quality order and source order.
- The status bar summarizes the active file's score and grade distribution.
- Diagnostics identify functions below your configured thresholds. Java
  classes and interfaces also receive class health scores and diagnostics.

Function health uses four pillars. Each pillar takes the lowest score among
its metrics, then the pillars are combined into an overall score.

| Pillar | Metrics |
| --- | --- |
| Control flow | Cognitive complexity and cyclomatic complexity |
| Size | Statements, measured as logical lines |
| Vocabulary load | Working memory and Halstead difficulty |
| Interface | Parameter count |

Scores use a 0–100 scale, with higher scores indicating better code health.
The four grades are **A** (80 or above), **B** (50 to below 80), **C** (25 to
below 50), and **D** (below 25).

Java class health uses weighted methods per class, public method count, and
public attribute count. The class scoring thresholds are based on Java
studies; C++ class metrics do not currently contribute to health scores.
The Explorer tree lists functions, while class details appear in hovers.

Use the scores to guide code review and refactoring. The
[scoring formulas, research references, and coverage limits][scoring] explain
how the measurements are combined and which language constructs are covered.

## Settings

Open Settings and search for `lspfAnalysis`, or use the Settings link in the
status bar hover. Scoring and diagnostic settings apply without restarting.

| Setting | Default | Meaning |
| --- | --- | --- |
| `lspfAnalysis.health.complexityThreshold` | 15 | Cognitive complexity scoring 50 |
| `lspfAnalysis.health.cyclomaticThreshold` | 10 | Cyclomatic complexity scoring 50 |
| `lspfAnalysis.health.lengthThreshold` | 30 | Statements scoring 50 |
| `lspfAnalysis.health.workingMemoryThreshold` | 8 | Names held at once scoring 50 |
| `lspfAnalysis.health.halsteadDifficultyThreshold` | 12 | Halstead difficulty scoring 50 |
| `lspfAnalysis.health.parametersThreshold` | 4 | Parameter count scoring 50 |
| `lspfAnalysis.health.wmcThreshold` | 34 | Weighted methods per Java class scoring 50 |
| `lspfAnalysis.health.publicMethodsThreshold` | 14 | Public methods per Java class scoring 50 |
| `lspfAnalysis.health.publicAttributesThreshold` | 8 | Public attributes per Java class scoring 50 |
| `lspfAnalysis.health.qualityWarn` | 25 | Report a warning below this score |
| `lspfAnalysis.health.qualityError` | 10 | Report an error below this score |
| `lspfAnalysis.diagnostics.enabled` | `true` | Show diagnostics |
| `lspfAnalysis.diagnostics.perMetric` | `false` | Add hints for a low pillar score even when overall quality passes |
| `lspfAnalysis.diagnostics.file` | `false` | Report files whose overall score falls below the error threshold |
| `lspfAnalysis.statusBar.enabled` | `true` | Show the active file's code health |

The `lspfAnalysis.health.weights.*` settings control relative contributions.
Control flow, size, and vocabulary load each default to `1`; interface and
class design each default to `0.5`. Class design controls the contribution of
Java classes to the file score.

For troubleshooting, `lspfAnalysis.trace.server` enables protocol tracing.
`lspfAnalysis.server.path` selects a custom server binary; leave it empty to
use the bundled server. Reload the window after changing this path.

## Commands

Search for `LSPF Analysis` in the Command Palette:

- **Restart Server** restarts the language server.
- **Sort by Quality** lists functions with the lowest scores first.
- **Sort by Position** lists functions in source order.

The same sorting commands are available in the Code Health toolbar. Function
links in the tree and status bar navigate directly to the corresponding line.

## Platform support

Packages target Windows, macOS, and Linux, with separate x64 and ARM64
binaries. Install the VSIX matching the operating system and architecture
where the extension runs. Linux packages target GNU/Linux, not Alpine/musl.
The extension requires a native language server and does not run directly in
the browser on vscode.dev.

## Support

Report bugs and request features in [GitHub Issues][issues]. Include the
extension version, VS Code version, operating system and architecture, and a
small source example that reproduces the problem. For startup problems,
check **View → Output → LSPF Analysis** and include the relevant log lines.
Remove private source code and sensitive paths before sharing logs.

Release notes are in the [changelog][changelog].

## Building it yourself

From `clients/vscode`, with the Rust and C toolchains installed:

```console
npm ci
npm run package
```

To select a target explicitly:

```console
npm run package -- --target darwin-arm64
```

Targets: `win32-x64`, `win32-arm64`, `darwin-x64`, `darwin-arm64`,
`linux-x64`, `linux-arm64`. Cross compilation needs the matching Rust target
and a C toolchain and linker for it, since the tree-sitter grammars are C.
Install the resulting VSIX through **Extensions → … → Install from VSIX**.

For a preview package, run `npm run package:pre-release` instead. See the
[publishing guide][publishing] for pre-release uploads and versioning.

## License

[MPL-2.0][license].

[readme-en]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/README.md
[readme-zh]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/README.zh-CN.md
[project]: https://github.com/meymchen/lspf-analysis
[scoring]: https://github.com/meymchen/lspf-analysis#what-it-reports
[issues]: https://github.com/meymchen/lspf-analysis/issues
[changelog]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/CHANGELOG.md
[license]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/LICENSE
[publishing]: https://github.com/meymchen/lspf-analysis/blob/main/clients/vscode/PUBLISHING.md
