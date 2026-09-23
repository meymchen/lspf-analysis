// Which Rust target builds the server for which VS Code platform.
//
// VS Code installs a platform-specific extension by matching its `--target`
// against the running machine, so every target here needs its own VSIX with
// its own binary inside. This mapping is the single source of truth for
// that correspondence.
//
// Linux builds link musl statically, so one binary runs on any distribution
// whatever its glibc, including an old one reached over Remote SSH. VS Code
// on Alpine asks for the alpine targets and does not fall back to linux, so
// those get the same binary in a VSIX of their own.

export const targets = {
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-musl',
  'linux-arm64': 'aarch64-unknown-linux-musl',
  'alpine-x64': 'x86_64-unknown-linux-musl',
  'alpine-arm64': 'aarch64-unknown-linux-musl',
};

/** Every VS Code target this extension is published for. */
export function vscodeTargets() {
  return Object.keys(targets);
}

/** Returns the Rust triple for a VS Code target, or throws if unsupported. */
export function rustTarget(vscodeTarget) {
  if (!Object.hasOwn(targets, vscodeTarget)) {
    throw new Error(
      `unsupported target ${vscodeTarget}; expected one of ${vscodeTargets().join(', ')}`,
    );
  }
  return targets[vscodeTarget];
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
