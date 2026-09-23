# Publishing LSPF Analysis

The extension is published as `meymchen.lspf-analysis` to the Visual Studio
Marketplace and to Open VSX. The first release is version `0.1.0` on the
pre-release channel.

Releases are built and published by the
[`release-vscode.yml`](../../.github/workflows/release-vscode.yml) workflow.
The manual steps further down are for local testing and for recovering from
a failed pipeline.

## Release a version

1. Set the version in `package.json` and `package-lock.json` together, for
   example with `npm version 0.1.1 --no-git-tag-version`.
2. Finalize the changelog entry. Use a heading such as `## 0.1.1` or
   `## 0.1.1 (preview)` and remove any `unreleased` marker. The changelog is
   included in the VSIX, so later edits do not reach that release.
3. Merge the change to `main`.
4. Tag the merged commit and push the tag:

   ```console
   git tag vscode-v0.1.1
   git push origin vscode-v0.1.1
   ```

The workflow then:

- checks that the tag matches the version, that the tagged commit is on
  `main`, and that the changelog has a finished entry for the version;
- runs the unit tests and builds a VSIX for each target: `win32-x64`,
  `win32-arm64`, `darwin-x64`, `darwin-arm64`, `linux-x64`, `linux-arm64`,
  `alpine-x64` and `alpine-arm64`;
- starts the packaged server on each target it builds natively;
- publishes all packages to the Marketplace and to Open VSX;
- creates a GitHub release with the packages, the changelog entry as notes,
  and a build provenance attestation.

An odd minor version such as `0.1.x` goes to the pre-release channel, and an
even one such as `0.2.x` to the stable channel. Both registries skip a
package whose version is already published, so re-running failed jobs after
a partial upload is safe.

A pull request that changes the workflow or the packaging scripts runs the
build jobs without publishing. To try the build at any other time, run the
workflow by hand from the Actions tab.

## One-time setup

The workflow needs the repository to be public: the `linux-arm64` build uses
a GitHub-hosted ARM runner, and the environment protection and attestations
it relies on are limited to paid plans for private repositories.

### GitHub

Create an environment named `vscode-release` under **Settings →
Environments**:

- Under **Deployment branches and tags**, allow only tags matching
  `vscode-v*`.
- Optionally, add yourself as a required reviewer, so every publish waits
  for approval.
- Add the variables `AZURE_CLIENT_ID` and `AZURE_TENANT_ID` from the Entra
  application below.
- Add the secret `OVSX_PAT` from Open VSX below.

A tag ruleset that restricts who can create `vscode-v*` tags adds a second
guard.

### Visual Studio Marketplace

Publishing uses Microsoft Entra ID through GitHub's OIDC token, so no
Marketplace token is stored anywhere.

1. In Microsoft Entra ID, register an application and add a federated
   credential for GitHub Actions with the entity type **Environment**,
   organization `meymchen`, repository `lspf-analysis` and environment
   `vscode-release`.
2. Make that application a Contributor on the `meymchen` publisher, as
   described in the [official guide][auth].

Global Azure DevOps PATs retire on December 1, 2026, so do not fall back to
`VSCE_PAT` for a new setup.

### Open VSX

1. Sign in to [open-vsx.org][open-vsx] with GitHub, and sign the Eclipse
   Foundation Open VSX Publisher Agreement in your profile.
2. Create an access token in your settings and create the namespace:

   ```console
   npx ovsx create-namespace meymchen -p <token>
   ```

3. Store the token as the `OVSX_PAT` environment secret.

Once the first version is published, switch to trusted publishing. It only
accepts a registration for an extension with a published version. Under
**Settings → Trusted Publishers**, register owner `meymchen`, repository
`lspf-analysis`, workflow `release-vscode.yml` and environment
`vscode-release`. Then delete the `OVSX_PAT` secret and revoke the token.
Without the secret, `ovsx` exchanges the job's OIDC token instead.

## Build a package locally

Run the commands below from `clients/vscode`. Use Node.js 22 or later, the
repository's Rust toolchain, and a C compiler and linker for the target.
Linux targets link musl statically and need `musl-gcc`, from the
`musl-tools` package on Debian and Ubuntu. On an ARM64 host, also set
`CC_aarch64_unknown_linux_musl=musl-gcc`.

```console
npm ci
npm test
npm run package:pre-release -- --target win32-x64
```

This produces `lspf-analysis-win32-x64-0.1.0-pre-release.vsix`, containing
the native language server and the bundled client. Omit `--target` to build
for the current machine. Build targets in separate workspaces when running
in parallel, because packaging replaces the shared `server/` directory.
`npm run package` builds the same package for the stable channel.

`package.json` keeps the numeric version `0.1.0`. The `-pre-release` suffix
belongs only to the filename. The script passes `--pre-release` to `vsce`,
which writes the pre-release property into the VSIX. The manifest's separate
`preview: true` field adds the Preview label; that field alone does not
select the pre-release channel.

## Publish by hand

If the pipeline cannot publish, download the packages from the workflow
run's artifacts or the GitHub release, then upload them from
`clients/vscode`:

```console
npx vsce publish --azure-credential --skip-duplicate --packagePath <vsix>...
npx ovsx publish --skip-duplicate --packagePath <vsix>... -p <token>
```

For `--azure-credential`, sign in with `az login` as an identity that is a
Contributor on the publisher. Upload the packages the pipeline built rather
than rebuilding them; the channel is recorded inside each VSIX, and renaming
a file does not change it. The [publisher management page][publishers] also
accepts a VSIX upload.

## Versioning

Use a new numeric version for each update. Marketplace versions use
`major.minor.patch`, without suffixes such as `-beta.1`. Use `0.1.x` for the
first previews and `0.2.0` for the first stable release. Before a stable
release, remove `preview: true`. The preview and stable releases must have
different versions. See the [official versioning guidance][pre-release] for
auto-update behavior.

After publishing, check the extension ID, version, channel, target platforms
and changelog on both registries. A package that needs a correction gets a
new version.

[auth]: https://code.visualstudio.com/api/working-with-extensions/publishing-extension#secure-automated-publishing-to-visual-studio-marketplace
[open-vsx]: https://open-vsx.org/
[publishers]: https://marketplace.visualstudio.com/manage/publishers/
[pre-release]: https://code.visualstudio.com/api/working-with-extensions/publishing-extension#prerelease-extensions
