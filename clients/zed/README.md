# LSPF Analysis for Zed

[简体中文](./README.zh-CN.md)

Find complex functions while editing. LSPF Analysis adds code health diagnostics
and Markdown hovers with scores and measurements for C++, Java, JavaScript
(including JSX), Python, Rust, TypeScript, and TSX. Java class health is included.
Your existing language server continues to provide completion and navigation.

This client uses Zed's standard LSP interface. File health status bars and
function trees from the other clients are outside its scope. Grade letters
remain readable without HTML colors. SSH and Dev Containers have not been
validated for this first release.

## Install locally

The extension is prepared for submission to the Zed extension directory.
Until the pinned server release is published, build a local server and configure
its path before opening a supported source file:

```console
cargo build --release --package lspf-analysis --locked
```

In Zed's Extensions view, choose **Install Dev Extension** and select this
repository's `clients/zed` directory. Install Rust with rustup first; Zed builds
the extension for `wasm32-wasip2`.

Add this to your Zed user settings, replacing the path with an absolute path
to your build. On Windows the filename ends in `.exe`; forward slashes work
in JSON paths on Windows too.

```json
{
  "lsp": {
    "lspf-analysis": {
      "binary": {
        "path": "C:/path/to/lspf-analysis/target/release/lspf-analysis.exe",
        "arguments": ["serve", "--stdio"]
      }
    }
  }
}
```

Always include `arguments` when overriding `binary.path`: Zed may bypass the
extension's command builder for a user-specified executable. Restart the
language server after changing its path or arguments.

Trust the project in Zed when prompted; Restricted Mode prevents language
servers from starting. Open a supported file and hover a function name.
Low scoring functions appear
in Zed's diagnostics. For Java, install a Java language extension if Zed does
not already recognize the file. This extension reuses existing language
definitions and supplies no grammars.

If you already set a language's `language_servers` list, add `lspf-analysis`
to that list. For example:

```json
{
  "languages": {
    "Rust": {
      "language_servers": ["rust-analyzer", "lspf-analysis", "..."]
    }
  }
}
```

`"..."` enables other registered servers too. Preserve any existing disabled
entries such as `"!server-name"`. The Zed language names are `C++`, `Java`,
`JavaScript`, `Python`, `Rust`, `TypeScript`, and `TSX`.

## Downloads and offline use

Without `binary.path`, the extension uses the server version recorded in
[`server-version`](./server-version), currently `0.1.0`. It downloads from the
`server-v0.1.0` GitHub release in `meymchen/lspf-analysis`, then reuses that exact
version from its extension work directory on later starts. It does not consult
the repository's latest release or implicitly select a program from `PATH`.

Downloads support Windows, macOS, and Linux on x64 and ARM64. Linux binaries
use musl. Files are downloaded to a temporary name and renamed after a
successful download; interrupted downloads are retried on the next start.
An existing complete cache requires no network access.

A first start without a cache requires access to GitHub release downloads.
If the release is unavailable, the network is offline, or Zed's extension
capabilities block downloads, the startup error includes the local-path remedy.
Check **zed: open log** for details. A local executable can also be used for an
older server version; its compatibility is then the user's responsibility.

Zed may log an unhandled `lspfAnalysis/fileHealth` notification. This is the
optional summary consumed by the other clients; diagnostics and hover use
standard LSP messages and do not depend on that notification.

## Settings

Use `lsp.lspf-analysis.settings.lspfAnalysis` in Zed's `settings.json`.
User settings apply globally; `.zed/settings.json` can override them for a
project. Keep analysis configuration in `settings`, rather than splitting it
between `settings` and `initialization_options`.

For Chinese output and a higher warning threshold:

```json
{
  "lsp": {
    "lspf-analysis": {
      "settings": {
        "lspfAnalysis": {
          "locale": "zh-cn",
          "health": { "qualityWarn": 50 }
        }
      }
    }
  }
}
```

English is the default. Display language does not follow Zed automatically.
Settings changes are sent to the running server and reanalyze open documents.
Removing an override restores the server default. An invalid field type is
logged by the server and leaves the last valid configuration active.

The [complete example](./settings.example.json) lists the analysis options
with their current defaults. Usually only the options you change are needed.
`diagnostics.enabled` controls diagnostics; `perMetric` adds advice when an
individual metric scores poorly; `file` adds file-level diagnostics.
`health.qualityWarn` and `qualityError` control severity cutoffs. Scores range
from 0 to 100; higher is healthier.

## Build and verify

Run from the repository root using its pinned Rust toolchain:

```console
rustup target add wasm32-wasip2
cargo fmt --manifest-path clients/zed/Cargo.toml -- --check
cargo clippy --manifest-path clients/zed/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path clients/zed/Cargo.toml --locked
cargo build --manifest-path clients/zed/Cargo.toml --target wasm32-wasip2 --release --locked
python clients/zed/scripts/check_extension.py
cargo build --package lspf-analysis --locked
python clients/zed/scripts/lsp_smoke.py target/debug/lspf-analysis
```

Append `.exe` to the final command's binary path on Windows. Python 3.11 or
later is required for the validation scripts. The smoke test checks diagnostics,
hover, JSX/TSX mapping, configuration changes, Chinese output, and shutdown
against the real server. It does not replace visual testing inside Zed.

The extension is an independent Cargo workspace, with an MIT license and its
own lockfile. It does not link the MPL-2.0 server into its WASM module.
See [PUBLISHING.md](./PUBLISHING.md) for server assets, the registry submission,
and the manual release checks.
