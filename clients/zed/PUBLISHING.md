# Publishing the Zed client

The Zed extension and native server have independent versions. The extension
version is in `extension.toml` and `Cargo.toml`; its tested server version is
in `server-version`. The server's own version is `workspace.package.version`
in the root `Cargo.toml`.

## Publish the server first

`.github/workflows/release-server.yml` builds and tests these native targets:

| System | Architecture | Rust target |
| --- | --- | --- |
| Windows | x64 | `x86_64-pc-windows-msvc` |
| Windows | ARM64 | `aarch64-pc-windows-msvc` |
| macOS | x64 | `x86_64-apple-darwin` |
| macOS | ARM64 | `aarch64-apple-darwin` |
| Linux | x64 | `x86_64-unknown-linux-musl` |
| Linux | ARM64 | `aarch64-unknown-linux-musl` |

Every target runs the real-server LSP smoke test on its matching native runner.
PR and manual workflow runs build and upload artifacts without publishing.
Use a manual run to rehearse the six-platform build before a release.

After merging and reviewing the checks, create and push `server-vX.Y.Z` on the
tested commit. The tag must match the root Cargo version, and that commit must
be reachable from `main`. Only a tag push can enable the publishing job.
Creating this tag is a release action, not a local build step.

The release provides six raw executable assets, for example:

```text
lspf-analysis-0.1.0-x86_64-pc-windows-msvc.exe
lspf-analysis-0.1.0-aarch64-apple-darwin
lspf-analysis-0.1.0-x86_64-unknown-linux-musl
```

`SHA256SUMS`, the server's MPL-2.0 `LICENSE`, and a `SOURCE.txt` link to the exact
source commit accompany them. Checksums can be used to verify manual downloads.
The extension uses fixed HTTPS release URLs and completed-download caching.

The workflow stages a draft release, uploads all assets, then publishes it.
It does not mark the server as the repository's latest release. If uploading
fails, inspect the draft and artifacts before retrying; an existing draft
is deliberately not overwritten by a rerun.

Confirm that all URLs derived from `server-version` are downloadable before
submitting an extension version that references them. For the first release,
use the local `binary.path` override while server assets are not public yet.

## Validate the extension commit

Run the build and verification commands in [README.md](./README.md).
CI also checks the independent extension dependency graph in the audit job.
The normal CI job compiles the WASM component, tests cache and configuration
behavior, and runs a real-server protocol smoke test. These checks do not
constitute visual testing of all six systems.

Before submitting the exact extension commit, manually verify in Zed:

- Install through **Install Dev Extension** from `clients/zed`.
- Open supported source files and verify that the existing language server's
  completion/navigation still works alongside LSPF Analysis.
- Hover function names and a Java class; check score tables and readable grades.
- Raise `qualityWarn`, observe a diagnostic, then disable diagnostics and check
  that it disappears without restarting the server.
- Set `locale` to `zh-cn` and check Chinese diagnostics and hover text.
- Test automatic download with a clean extension work directory, then restart
  offline with the cache. Keep a backup rather than deleting a working cache.
- Test a local path with spaces, using `["serve", "--stdio"]` as arguments.
- Record the exact commit, Zed version, OS/architecture, and which checks passed.

State which platforms have only native CI verification and which were tested
interactively. SSH and Dev Container validation are deferred for this release.

### Initial local verification

The initial implementation was checked as an uncommitted working tree on
Windows x64, with Zed 1.22.0 and Rust 1.98.0:

- The server workspace tests, Clippy, and formatting passed.
- The extension's cache/configuration tests, Clippy, WASI component build,
  and manifest checks passed. Its dependency audit passed.
- Both debug and staged release servers passed the eight-file-type LSP smoke
  test, including live configuration and Chinese hover text.
- In an isolated Zed profile, the real extension loaded, launched the staged
  server from its versioned cache, initialized, opened a Rust file, and published
  diagnostics. Editing project settings triggered another configuration
  notification and diagnostic publication without restarting the server.
- Release-script tests, actionlint, zizmor, Python checks, and formatting of
  the changed source files passed.

The other five native targets have a CI matrix but were not built locally.
An actual first download awaits publication of the pinned server assets.
Hover rendering, completion coexistence, and the remaining manual checks above
still need interactive verification on the exact commit submitted to Zed.
Repository-wide Prettier also finds existing Visual Studio `obj` artifacts;
those generated files are outside this change and were left untouched.

## Submit to the extension directory

The extension remains in this monorepo. In a checkout of your fork of
`zed-industries/extensions`, add this repository as a submodule at
`extensions/lspf-analysis`, pin it to the tested commit, and register:

```toml
[lspf-analysis]
submodule = "extensions/lspf-analysis"
path = "clients/zed"
version = "0.1.0"
```

The registry version must equal `clients/zed/extension.toml` at that commit.
Run the registry's prescribed formatting and checks, then open its submission
PR. The `LICENSE` inside `clients/zed` is MIT and covers the extension code;
the downloaded server is a separate MPL-2.0 program. No server executable or
generated WASM should be committed to this source directory.

Suggested submission description:

> Adds LSPF Analysis, a secondary language server for C++, Java, JavaScript,
> Python, Rust, TypeScript, and TSX. It provides code health diagnostics and
> score hovers, downloads a pinned native server, and supports a local binary
> override. The extension lives at `clients/zed` in its monorepo and is MIT
> licensed. Attach the exact manual verification record before submission.

## Updates

Change the extension version in both manifests and describe the change in
`CHANGELOG.md`. To update the default server, publish its release first, update
`server-version`, then rerun the extension checks and manual verification.
Update the registry submodule commit and matching version through another PR.
Older complete caches may remain in the extension work directory; the extension
only selects the version it declares.

Official references:

- [Developing extensions](https://zed.dev/docs/extensions/developing-extensions)
- [Publishing prerequisites](https://zed.dev/docs/extensions/publishing/prerequisites)
- [License requirements](https://zed.dev/docs/extensions/publishing/license-requirements)
- [Publishing guide](https://zed.dev/docs/extensions/publishing/publishing-guide)
- [Updating extensions](https://zed.dev/docs/extensions/publishing/updating-and-maintenance)
