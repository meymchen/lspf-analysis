# LSPF Analysis for IntelliJ IDEA

Tells you, while you type, which functions are getting hard to work with.

This plugin bundles the [lspf-analysis](https://github.com/meymchen/lspf-analysis)
language server, so nothing else has to be installed — no Rust toolchain, no
generic LSP client.

Supported languages: **C++**, **Java**, **JavaScript**, **Python**, **Rust**,
**TypeScript**, **TSX**.

Everything it displays follows the IDE's own display language: Simplified
Chinese when the IDE is in Simplified Chinese, English otherwise. That includes
the hovers and the Problems entries, which the language server renders — the
plugin tells it which language you read.

## Requirements

**2026.1.4 or later**, any IntelliJ-based IDE. That is the release which
open-sourced the platform's LSP client, the one this plugin uses; before it, LSP
integration was a commercial-IDE extension and a plugin built on it was silently
inert in Community builds and Android Studio.

Only the Windows x64 build ships with a server binary today. On any other
platform the plugin works the same, but you have to build the server yourself
(`cargo install --path crates/lspf-analysis`) and point **Language server path**
at it.

## What it reports

Hover the LSPF Analysis icon beside a function or class in the editor gutter to
see its measurements. Hovering the symbol's name keeps the IDE's original
documentation available. A function below the quality threshold gets a warning
under its name.

The status bar carries the file as a whole. Its tooltip shows the quality bar
and how the file's functions are spread across the four bands; clicking it opens
the ones worth opening first — each a row that jumps to the line, alongside
shortcuts to the Problems view, the settings, and a server restart.

**Function Health**, on the right, lists every function of the file at once,
worst first, rather than one at a time under the pointer. Clicking a row jumps
to it; expanding one shows the four pillars, and expanding a pillar shows each
metric against the threshold it is judged by. The toolbar toggles between
worst-first and source order.

Switching files clears the previous rows and requests the new file immediately.
Updates to the same file keep the rows visible while waiting for refreshed
results, with a one-second delay after the last health publication. An empty
answer clears the rows. A failed request or a ten-second timeout shows
"Function Health unavailable" until the next refresh. Stopping the server also
clears the rows; initialization requests the active file again immediately.

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

**Settings | Tools | LSPF Analysis**. Everything takes effect on Apply, without
a restart; changing the server path restarts the server, since a different
binary is a different server.

Thresholds, weights and diagnostics are stored per project — a parser and a
controller are not held to the same complexity. The server path is stored per
IDE, because it follows the installation rather than the codebase.

| Setting | Default | Meaning |
| --- | --- | --- |
| Cognitive complexity | 15 | Cognitive complexity scoring 50% |
| Cyclomatic complexity | 10 | Cyclomatic complexity scoring 50% |
| Statements | 30 | Statements scoring 50% |
| Working memory | 8 | Names held at once scoring 50% |
| Halstead difficulty | 12 | Halstead difficulty scoring 50% |
| Parameters | 4 | Parameters scoring 50% |
| Weighted methods per class | 34 | WMC scoring 50% |
| Public methods per class | 14 | Public methods scoring 50% |
| Public attributes per class | 8 | Public attributes scoring 50% |
| Weights | 1 / 1 / 1 / 0.5 / 0.5 | Relative pillar weights, plus class design |
| Warn below | 25 | Below this, a function is reported |
| Error below | 10 | Below this, it is an error |
| Publish diagnostics | on | Publish anything at all |
| Also hint at one badly scoring pillar | off | For a function that passes overall |
| Also report the file as a whole | off | When the file is bad enough to be an error |
| Show the file's quality in the status bar | on | |
| Show health icons in the editor gutter | on | Hover for function or class measurements |
| Language server path | *(empty)* | Use this binary instead of the bundled one |

There is no trace switch: the platform owns the LSP conversation and logs it
itself. Turn on *Show in tool window* for the **LSP log: info, trace** category
in **Settings | Appearance & Behavior | Notifications**.

## Actions

- **Restart Server**
- **Sort by Quality** — order Function Health worst first
- **Sort by Position** — order it by where the functions are

The two sort actions appear in the Function Health toolbar one at a time:
whichever order the view is not currently in.

## Building it yourself

From `clients/intellij`, with a JDK 21 — which Gradle downloads itself if you
have none — and a Rust toolchain on the path:

```console
./gradlew test          # model and IntelliJ integration tests
./gradlew buildPlugin   # build/distributions/lspf-analysis-intellij-win32-x64-<version>.zip
./gradlew runIde        # IntelliJ IDEA sandbox with the plugin installed
./gradlew runPyCharm    # PyCharm sandbox with the plugin installed
./gradlew verifyPlugin  # IntelliJ Plugin Verifier against the supported range
```

`buildPlugin` runs `cargo build --release` for the packaged target and puts the
binary in `server/` inside the ZIP.

`runIde` never does that. A sandbox IDE runs `target/debug/lspf-analysis` from
the repository instead, so `runIde` builds only that — a plain host `cargo build`
— and skips the release build it would have no use for.

Other platforms are one line in the `serverTargets` map in `build.gradle.kts`
plus a ZIP to publish. A cross build also needs `rustup target add` for the
matching triple and a C toolchain for it, since the tree-sitter grammars are C.

### Debugging the plugin

Choose **Run Plugin** for IntelliJ IDEA or **Run Plugin (PyCharm)** for PyCharm,
then click Debug. Both run version 2026.1.4 with the plugin installed and build
the debug language server from this checkout. Gradle downloads the selected IDE
on first use and keeps each IDE's sandbox separate. Kotlin breakpoints attach
to the sandbox IDE, and its `idea.log` includes the server's stderr.

From a terminal in `clients/intellij`, use `./gradlew runIde` or
`./gradlew runPyCharm`. On Windows, use `gradlew.bat`. Add `--debug-jvm` to wait
for a JVM debugger on port 5005.

Nothing about that needs the TCP setup below. TCP exists only for stepping
through the *server*, which is a separate Rust process the debugger cannot reach
through the plugin.

### Running against a server you started yourself

The TCP path is for when you want to watch the server's own output as it runs,
restart it without restarting the sandbox, or attach a native debugger to it.
Two configurations, **in this order**:

1. **LSP Server (TCP)** — the `runServerTcp` Gradle task: builds the debug
   server, then runs it on `127.0.0.1:9257` with `RUST_LOG=debug`. Override
   either with `-PtcpAddress=` / `-PrustLog=`.

   A Gradle task rather than a shell script on purpose. A Shell Script
   configuration renders its environment variables as `export FOO=bar;`, which
   is a syntax error in PowerShell — so it would need a different command per
   OS. Gradle's `Exec` puts the variable straight into the child process, with
   no shell in between to disagree.
2. **Run Plugin (TCP)** for IntelliJ IDEA or **Run Plugin (PyCharm, TCP)** for
   PyCharm — `-PdebugPort=9257` makes the
   plugin connect to that socket instead of spawning a process of its own.

The order matters, and there is deliberately no compound configuration that
starts both at once, for two reasons that compound each other:

- When the IDE is told a server is already running
  (`LspCommunicationChannel.Socket(port, startProcess = false)`), the platform
  attempts the connection exactly once and does not retry. So the server has to
  be listening first — wait for `listening for one TCP client` in the terminal
  before opening a source file in the sandbox.
- `serve --tcp` means exactly that: the server takes one client and exits when
  it disconnects. Every reconnection needs a fresh server.

So the recovery from a lost or mistimed connection is to run **LSP Server (TCP)**
again and then **Restart Server** inside the sandbox IDE — in that order. The
sandbox itself never has to be restarted.

Running **Run Plugin (TCP)** on its own therefore fails with `connection
refused`: it is asking for a server nobody started. That is the configuration
working as intended, not a fault — use plain **Run Plugin** unless you actually
want a server of your own.

Breakpoints on the Rust side additionally need the Rust plugin, which is
IntelliJ IDEA Ultimate or RustRover only — it declares a dependency on
`com.intellij.modules.ultimate` (and, since 2026.2, on `com.intellij.nativeDebug`).
With a Community subscription that plugin cannot load at all, which is why the
server is started from a terminal here rather than from a Cargo configuration.

## License

[MPL-2.0](./LICENSE).
