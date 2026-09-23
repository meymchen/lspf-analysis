// Builds the language server and drops the binary into `server/`, which is
// what ends up inside the VSIX.
//
// The build is always given an explicit `--target`, even for the host, so
// that the artifact path is the same whether or not this is a cross build
// and a stale host binary can never be mistaken for a cross-built one.

import { chmod, copyFile, mkdir, rm, stat } from 'node:fs/promises';
import * as path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { hostVscodeTarget, rustTarget } from './targets.mjs';

export const extensionRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const repositoryRoot = path.resolve(extensionRoot, '..', '..');

/** The server binary's name on a given platform. */
export function binaryName(vscodeTarget) {
  return vscodeTarget.startsWith('win32-') ? 'lspf-analysis.exe' : 'lspf-analysis';
}

/**
 * Reads `--target <t>` and `--skip-build` out of command line arguments.
 *
 * `--skip-build` exists for a caller that has already produced the binary,
 * such as a CI job that builds and packages in separate steps.
 */
export function parseArguments(argv) {
  const options = { target: undefined, skipBuild: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--skip-build') {
      options.skipBuild = true;
    } else if (argument === '--target') {
      options.target = argv[index + 1];
      if (!options.target || options.target.startsWith('--')) {
        throw new Error('--target requires a platform, for example win32-x64');
      }
      index += 1;
    } else if (argument.startsWith('--target=')) {
      options.target = argument.slice('--target='.length);
    } else {
      throw new Error(`unrecognized argument ${argument}`);
    }
  }
  if (options.target === undefined) {
    options.target = hostVscodeTarget();
  }
  rustTarget(options.target);
  return options;
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: 'inherit',
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} exited with ${result.status}`);
  }
}

/**
 * What to suggest when a build for `triple` does not work out.
 *
 * Both usual causes are invisible in cargo's own output when the target is
 * simply not set up on this machine.
 */
function crossBuildHint(triple) {
  return (
    `Install the target with: rustup target add ${triple}\n` +
    'A cross build also needs a C toolchain and linker for the target, ' +
    'because the tree-sitter grammars are C.'
  );
}

/** Builds for `target` and returns the path of the binary now in `server/`. */
export async function prepareServer({ target, skipBuild }) {
  const triple = rustTarget(target);
  if (!skipBuild) {
    try {
      run(
        'cargo',
        ['build', '--locked', '--release', '--package', 'lspf-analysis', '--target', triple],
        repositoryRoot,
      );
    } catch (error) {
      throw new Error(`${error.message}\n${crossBuildHint(triple)}`);
    }
  }

  const executable = binaryName(target);
  const source = path.join(repositoryRoot, 'target', triple, 'release', executable);
  try {
    await stat(source);
  } catch {
    throw new Error(`no server binary at ${source}.\n${crossBuildHint(triple)}`);
  }

  // Clear the directory first: a binary left over from another target would
  // otherwise be packaged alongside this one.
  const destinationDirectory = path.join(extensionRoot, 'server');
  await rm(destinationDirectory, { recursive: true, force: true });
  await mkdir(destinationDirectory, { recursive: true });

  const destination = path.join(destinationDirectory, executable);
  await copyFile(source, destination);
  if (process.platform !== 'win32') {
    await chmod(destination, 0o755);
  }
  return destination;
}

/**
 * Reports a failure as a message rather than a stack trace: everything
 * thrown here is a build problem for the caller to act on, not a bug in
 * this script.
 */
export function fail(error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const options = parseArguments(process.argv.slice(2));
    const destination = await prepareServer(options);
    console.log(`${options.target}: ${path.relative(extensionRoot, destination)}`);
  } catch (error) {
    fail(error);
  }
}
