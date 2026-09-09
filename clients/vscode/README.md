# LSPF Analysis for VS Code

Tells you, while you type, which functions are getting hard to work with.

This extension bundles the [lspf-analysis](https://github.com/meymchen/lspf-analysis)
language server, so nothing else has to be installed — no Rust toolchain, no
generic LSP client.

Supported languages: **C++**, **Java**, **JavaScript**, **Python**, **Rust**,
**TypeScript**, **TSX**.

Everything it displays follows VS Code's own display language: Simplified
Chinese when the editor is in Simplified Chinese, English otherwise. That
includes the hovers and the Problems entries, which the language server
renders — the extension tells it which language you read.

## What it reports

Hovering a function's name shows its measurements; a function below the
quality threshold gets a diagnostic under its name. The hover appears
on the name and nowhere else, so it adds to what the editor already tells you
about a symbol instead of displacing it.

The status bar carries the file as a whole. Its hover shows the quality bar,
how the file's functions are spread across the four bands, and the ones worth
opening first — each a link that jumps to the line, alongside shortcuts to the
Problems view and the thresholds. Turn the item off with
`lspfAnalysis.statusBar.enabled`.

**Function Health**, in the Explorer, lists every function of the file at
once, worst first, rather than one at a time under the pointer. Clicking a
row jumps to it; expanding one shows the four pillars, and expanding a pillar
shows each metric against the threshold it is judged by. The title bar toggles
between worst-first and source order.

Quality blends four pillars, each taking the worst of its metrics:

| Pillar | What it measures | Metrics |
| --- | --- | --- |
| **Control flow** | How tangled the control flow is | cognitive and cyclomatic complexity |
| **Size** | How much a function does | statements (logical lines) |
| **Vocabulary load** | How many names you hold at once | working memory, Halstead difficulty |
| **Interface** | How wide the signature is | parameters |

Quality maps to a band: **excellent** (≥80), **good** (≥50), **fair** (≥25),
**poor** (<25). The formula, the thresholds and the papers behind each choice
— including which metrics are deliberately *not* scored, and why — are in the
[project README](https://github.com/meymchen/lspf-analysis#what-it-reports).

## Settings

Every setting lives under `lspfAnalysis` and takes effect immediately, without
a restart.

| Setting | Default | Meaning |
| --- | --- | --- |
| `lspfAnalysis.health.complexityThreshold` | 15 | Cognitive complexity scoring 50% |
| `lspfAnalysis.health.cyclomaticThreshold` | 10 | Cyclomatic complexity scoring 50% |
| `lspfAnalysis.health.lengthThreshold` | 30 | Statements scoring 50% |
| `lspfAnalysis.health.workingMemoryThreshold` | 8 | Names held at once scoring 50% |
| `lspfAnalysis.health.halsteadDifficultyThreshold` | 12 | Halstead difficulty scoring 50% |
| `lspfAnalysis.health.parametersThreshold` | 4 | Parameters scoring 50% |
| `lspfAnalysis.health.weights.*` | 1 / 1 / 1 / 0.5 | Relative pillar weights |
| `lspfAnalysis.health.qualityWarn` | 25 | Below this, a function is reported |
| `lspfAnalysis.health.qualityError` | 10 | Below this, it is an error |
| `lspfAnalysis.diagnostics.enabled` | true | Publish diagnostics at all |
| `lspfAnalysis.diagnostics.perMetric` | false | Also hint at one badly scoring pillar |
| `lspfAnalysis.diagnostics.file` | false | Also report the file as a whole |
| `lspfAnalysis.statusBar.enabled` | true | Show the file's quality in the status bar |
| `lspfAnalysis.server.path` | `""` | Use this binary instead of the bundled one |
| `lspfAnalysis.trace.server` | `off` | Trace the LSP messages |

## Commands

- **LSPF Analysis: Restart Server**
- **LSPF Analysis: Sort by Quality** — order Function Health worst first
- **LSPF Analysis: Sort by Position** — order it by where the functions are

`LSPF Analysis: Go to Function` also exists, but takes arguments and is only
ever invoked from a link in the status bar hover or a row of the tree, so it
is hidden from the command palette.

## Building it yourself

The extension is packaged per platform, each VSIX carrying the server binary
for one architecture. From `clients/vscode`:

```console
npm ci
npm run package                             # for this machine
npm run package -- --target darwin-arm64    # for another platform
```

Targets: `win32-x64`, `win32-arm64`, `darwin-x64`, `darwin-arm64`,
`linux-x64`, `linux-arm64`. Building for a platform other than the host needs
`rustup target add` for the matching triple and a C toolchain for it, since
the tree-sitter grammars are C.

## License

[MPL-2.0](./LICENSE).
