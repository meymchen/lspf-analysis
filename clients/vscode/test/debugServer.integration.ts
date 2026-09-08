import * as assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import * as path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';
import {
  createMessageConnection,
  StreamMessageReader,
  StreamMessageWriter,
} from 'vscode-jsonrpc/node';

import { connectToServer, connectToWebSocketServer } from '../src/debug.js';
import { isFileHealth } from '../src/status.js';
import { stdioArgs } from '../src/serverPath.js';

const root = path.resolve(__dirname, '../../../..');
const binary = path.join(
  root,
  'target/debug',
  `lspf-analysis${process.platform === 'win32' ? '.exe' : ''}`,
);

for (const protocol of ['stdio', 'tcp', 'ws'] as const) {
  test(
    `extension debug transport initializes and shuts down a real ${protocol} server`,
    { timeout: 15000 },
    async (t) => {
      const env: NodeJS.ProcessEnv = { ...process.env, RUST_LOG: 'info' };
      delete env.LSPF_ANALYSIS_LOG_FILE;
      const args = protocol === 'stdio' ? stdioArgs() : ['serve', `--${protocol}`, '127.0.0.1:0'];
      const server = spawn(binary, args, {
        cwd: root,
        env,
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true,
      });
      const exited = once(server, 'exit');
      t.after(() => {
        if (server.exitCode === null) server.kill();
      });
      let logs = '';
      server.stderr.on('data', (chunk) => {
        logs += chunk;
      });
      const port =
        protocol === 'stdio'
          ? undefined
          : await new Promise<number>((resolve, reject) => {
              server.once('error', reject);
              server.once('exit', (code) =>
                reject(new Error(`server exited before listening: ${code}\n${logs}`)),
              );
              server.stderr.on('data', () => {
                const match =
                  /listening for one (?:TCP|WebSocket) client bound=127\.0\.0\.1:(\d+)/.exec(logs);
                if (match) resolve(Number(match[1]));
              });
            });
      const transports =
        port === undefined
          ? {
              reader: new StreamMessageReader(server.stdout),
              writer: new StreamMessageWriter(server.stdin),
            }
          : protocol === 'ws'
            ? await connectToWebSocketServer(port)
            : await (async () => {
                const streams = await connectToServer(port);
                t.after(() => {
                  (streams.writer as import('node:net').Socket).destroy();
                });
                return {
                  reader: new StreamMessageReader(streams.reader),
                  writer: new StreamMessageWriter(streams.writer),
                };
              })();
      const connection = createMessageConnection(transports.reader, transports.writer);
      t.after(() => connection.dispose());
      connection.listen();
      const result = await connection.sendRequest<{ capabilities: object }>('initialize', {
        processId: process.pid,
        rootUri: null,
        capabilities: {},
      });
      assert.ok(result.capabilities);
      await connection.sendNotification('initialized', {});
      assert.equal(await connection.sendRequest('shutdown'), null);
      await connection.sendNotification('exit');
      assert.deepEqual(await exited, [0, null], logs);
    },
  );
}

/**
 * The stdio session an F5 actually runs, as far as something reported on screen.
 *
 * The transport tests above stop at `initialize`, which leaves the whole of
 * what a reader sees — the status bar summary, the hover — untested against a
 * real process. This opens a document the way the editor does and insists the
 * server answers with both, under `RUST_LOG=debug`: stdout carries the
 * protocol and stderr carries the log, and a server that mixed them would
 * take the session down without either being obviously at fault.
 */
test(
  'a real stdio server reports file health and hovers a function',
  { timeout: 15000 },
  async (t) => {
    const env: NodeJS.ProcessEnv = { ...process.env, RUST_LOG: 'debug' };
    delete env.LSPF_ANALYSIS_LOG_FILE;
    const server = spawn(binary, stdioArgs(), {
      cwd: root,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
    t.after(() => {
      if (server.exitCode === null) server.kill();
    });
    let logs = '';
    server.stderr.on('data', (chunk) => {
      logs += chunk;
    });

    const connection = createMessageConnection(
      new StreamMessageReader(server.stdout),
      new StreamMessageWriter(server.stdin),
    );
    t.after(() => connection.dispose());
    const notified: Array<{ method: string; params: unknown }> = [];
    connection.onNotification((method, params) => notified.push({ method, params }));
    connection.listen();

    await connection.sendRequest('initialize', {
      processId: process.pid,
      rootUri: pathToFileURL(root).href,
      capabilities: { general: { positionEncodings: ['utf-16'] } },
      initializationOptions: { lspfAnalysis: { locale: 'en' } },
    });
    await connection.sendNotification('initialized', {});

    const uri = pathToFileURL(path.join(root, 'stdio-probe.rs')).href;
    const text =
      'fn wide(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 {\n    a + b + c + d + e\n}\n';
    await connection.sendNotification('textDocument/didOpen', {
      textDocument: { uri, languageId: 'rust', version: 1, text },
    });

    const health = await Promise.race([
      new Promise<unknown>((resolve) => {
        const found = () => {
          const summary = notified.find((one) => one.method === 'lspfAnalysis/fileHealth');
          if (summary) resolve(summary.params);
          else setTimeout(found, 25);
        };
        found();
      }),
      new Promise((_, reject) =>
        setTimeout(() => reject(new Error(`no file health over stdio\n${logs}`)), 10000),
      ),
    ]);
    // The client drops a summary it does not recognize, which is the quiet
    // way this breaks: the status bar simply never appears.
    assert.ok(isFileHealth(health), `${JSON.stringify(health)}\n${logs}`);
    assert.equal(health.functions, 1);

    // `fn wide(`: the name starts at column 3.
    const hover = await connection.sendRequest<{ contents: { value: string } } | null>(
      'textDocument/hover',
      { textDocument: { uri }, position: { line: 0, character: 4 } },
    );
    assert.ok(hover, `no hover over stdio\n${logs}`);
    assert.match(hover.contents.value, /\bwide\b/);
    assert.match(hover.contents.value, /parameters \| 5 \/ 4/);

    assert.equal(await connection.sendRequest('shutdown'), null);
    await connection.sendNotification('exit');
  },
);
