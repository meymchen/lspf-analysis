// Produces one platform-specific VSIX: the server binary for the target,
// plus the bundled extension code.
//
// With no `--target` this packages for the machine it runs on, which is the
// common case while developing. `vsce` runs `vscode:prepublish`, so the
// esbuild bundle is refreshed on the way through.
// `--pre-release` marks the VSIX for the Marketplace pre-release channel.

import { readFile } from 'node:fs/promises';
import * as path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

import { extensionRoot, fail, parseArguments, prepareServer } from './prepareServer.mjs';

const require = createRequire(import.meta.url);

try {
  const argv = process.argv.slice(2);
  const preRelease = argv.includes('--pre-release');
  const options = parseArguments(argv.filter((argument) => argument !== '--pre-release'));
  await prepareServer(options);

  const manifest = JSON.parse(await readFile(path.join(extensionRoot, 'package.json'), 'utf8'));
  const suffix = preRelease ? '-pre-release' : '';
  const output = `${manifest.name}-${options.target}-${manifest.version}${suffix}.vsix`;
  const args = [
    require.resolve('@vscode/vsce/vsce'),
    'package',
    '--target',
    options.target,
    '--no-dependencies',
    '--out',
    output,
  ];
  if (preRelease) {
    args.push('--pre-release');
  }

  // Run the locked local CLI through Node without a shell, including on Windows.
  const result = spawnSync(process.execPath, args, { cwd: extensionRoot, stdio: 'inherit' });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
  console.log(`packaged ${path.join(extensionRoot, output)}`);
} catch (error) {
  fail(error);
}
