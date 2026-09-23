# Publishing LSPF Analysis

The first Marketplace release is prepared as version `0.1.0` on the
pre-release channel, under publisher `meymchen`.

Run the commands below from `clients/vscode`. Use Node.js 22 or later, the
repository's Rust toolchain, and a C compiler and linker for the target.

## Build the preview package

Before packaging, finalize the changelog entry for the version being built.
Use a heading such as `0.1.0 (preview)` and remove any `unreleased` marker from
that entry. The changelog is included in the VSIX, so changes made afterward
will not update the package or its Marketplace release notes. Add a release
date only when it is known.

```console
npm ci
npm test
npm run package:pre-release -- --target win32-x64
```

This produces `lspf-analysis-win32-x64-0.1.0-pre-release.vsix`, containing the
native language server and the bundled client. Omit `--target` to build for
the current machine. The other supported targets are `win32-arm64`,
`darwin-x64`, `darwin-arm64`, `linux-x64`, and `linux-arm64`; each needs its
own matching native build. Build targets in separate workspaces when running
in parallel, because packaging replaces the shared `server/` directory.

The build uses `Cargo.lock` and the locally installed `vsce`. Install the
VSIX in a clean VS Code profile and check startup, hovers, diagnostics, and
Code Health navigation before uploading that same file.

`package.json` keeps the numeric version `0.1.0`. The `-pre-release` suffix
belongs only to the filename. The script passes `--pre-release` to `vsce`,
which writes the pre-release property into the VSIX. The manifest's separate
`preview: true` field adds the Preview label; that field alone does not
select the pre-release channel.

`npm run package` remains available for packaging without the pre-release
channel flag. Its VSIX has a different filename. Use `package:pre-release`
for this preview release.

## Upload an existing preview package

For a first manual upload, sign in to the
[Marketplace publisher management page][publishers], select `meymchen`, and
upload the generated `-pre-release.vsix`. Keep the pre-release marker in
the package; renaming a regular VSIX does not change its channel.

For CLI publishing, Microsoft recommends Microsoft Entra ID authentication.
Configure the publishing identity as a Contributor on the Marketplace
publisher, authenticate it as described in the [official guide][auth], then
run:

```console
npm run publish:pre-release -- ./lspf-analysis-win32-x64-0.1.0-pre-release.vsix --azure-credential
```

This command uploads the supplied package. It does not build another package
or change the version. A VSIX path is required; `vsce` rejects a package that
was not built with `--pre-release`. Multiple package paths can be supplied
for different platforms of the same version.

If you already use an Azure DevOps PAT, `vsce` can read it from the `VSCE_PAT`
environment variable when `--azure-credential` is omitted. Do not put a token
in a script, command argument, or committed file. Global Azure DevOps PATs
retire on December 1, 2026; prefer Entra ID for a new publishing setup.

Push the README, translations, changelog, and icon to the GitHub `main`
branch before publishing so the Marketplace's documentation links resolve.
Once uploaded, verify the extension ID `meymchen.lspf-analysis`, version,
pre-release channel, target platform, and changelog in Marketplace. If an
uploaded package needs a correction, create a new version and rebuild it.

## Subsequent releases

Use a new numeric version for each update, and update `package.json` and
`package-lock.json` together. Marketplace versions use `major.minor.patch`,
without suffixes such as `-beta.1`.

Use `0.1.x` for the first previews and `0.2.0` for the first stable release.
Before a stable release, remove `preview: true`, package without
`--pre-release`, and publish the resulting stable VSIX. The preview and
stable releases must have different versions. See the
[official versioning guidance][pre-release] for auto-update behavior.

[publishers]: https://marketplace.visualstudio.com/manage/publishers/
[auth]: https://code.visualstudio.com/api/working-with-extensions/publishing-extension#secure-automated-publishing-to-visual-studio-marketplace
[pre-release]: https://code.visualstudio.com/api/working-with-extensions/publishing-extension#prerelease-extensions
