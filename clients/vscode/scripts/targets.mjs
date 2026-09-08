// Which Rust target builds the server for which VS Code platform.
//
// VS Code installs a platform-specific extension by matching its `--target`
// against the running machine, so every target here needs its own VSIX with
// its own binary inside. This mapping is the single source of truth for
// that correspondence.

export const targets = {
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
};

/** Every VS Code target this extension is published for. */
export function vscodeTargets() {
  return Object.keys(targets);
}

/** Returns the Rust triple for a VS Code target, or throws if unsupported. */
export function rustTarget(vscodeTarget) {
  const triple = targets[vscodeTarget];
  if (!triple) {
    throw new Error(
      `unsupported target ${vscodeTarget}; expected one of ${vscodeTargets().join(', ')}`,
    );
  }
  return triple;
}

/**
 * Names the VS Code target of the machine running this script.
 *
 * Packaging without `--target` builds for the host, which is what a
 * developer wants when they just need an installable VSIX.
 */
export function hostVscodeTarget(platform = process.platform, arch = process.arch) {
  const candidate = `${platform}-${arch}`;
  if (!(candidate in targets)) {
    throw new Error(
      `no VS Code target for ${candidate}; pass --target explicitly, one of ${vscodeTargets().join(', ')}`,
    );
  }
  return candidate;
}
