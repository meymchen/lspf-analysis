// Produces one platform-specific VSIX: the server binary for the target,
// plus the bundled extension code.
//
// With no `--target` this packages for the machine it runs on, which is the
// common case while developing. `vsce` runs `vscode:prepublish`, so the
// esbuild bundle is refreshed on the way through.

import { readFile } from 'node:fs/promises';
import * as path from 'node:path';
import { spawnSync } from 'node:child_process';

import { extensionRoot, fail, parseArguments, prepareServer } from './prepareServer.mjs';

try {
  const options = parseArguments(process.argv.slice(2));
  await prepareServer(options);

  const manifest = JSON.parse(await readFile(path.join(extensionRoot, 'package.json'), 'utf8'));
  const output = `${manifest.name}-${options.target}-${manifest.version}.vsix`;

  const result = spawnSync(
    'npx',
    ['vsce', 'package', '--target', options.target, '--no-dependencies', '--out', output],
    { cwd: extensionRoot, stdio: 'inherit', shell: process.platform === 'win32' },
  );
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
