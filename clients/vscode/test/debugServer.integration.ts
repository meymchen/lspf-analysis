import * as assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import * as path from 'node:path';
import test from 'node:test';
import { createMessageConnection, StreamMessageReader, StreamMessageWriter } from 'vscode-jsonrpc/node';

import { connectToServer, connectToWebSocketServer } from '../src/debug.js';

const root = path.resolve(__dirname, '../../../..');
const binary = path.join(root, 'target/debug', `lspf-analysis${process.platform === 'win32' ? '.exe' : ''}`);

for (const protocol of ['stdio', 'tcp', 'ws'] as const) {
    test(`extension debug transport initializes and shuts down a real ${protocol} server`, { timeout: 15000 }, async t => {
        const env: NodeJS.ProcessEnv = { ...process.env, RUST_LOG: 'info' };
        delete env.LSPF_ANALYSIS_LOG_FILE;
        const args = protocol === 'stdio' ? ['serve', '--stdio'] : ['serve', `--${protocol}`, '127.0.0.1:0'];
        const server = spawn(binary, args, {
            cwd: root, env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true,
        });
        const exited = once(server, 'exit');
        t.after(() => { if (server.exitCode === null) server.kill(); });
        let logs = '';
        server.stderr.on('data', chunk => { logs += chunk; });
        const port = protocol === 'stdio' ? undefined : await new Promise<number>((resolve, reject) => {
            server.once('error', reject);
            server.once('exit', code => reject(new Error(`server exited before listening: ${code}\n${logs}`)));
            server.stderr.on('data', () => {
                const match = /listening for one (?:TCP|WebSocket) client bound=127\.0\.0\.1:(\d+)/.exec(logs);
                if (match) resolve(Number(match[1]));
            });
        });
        const transports = port === undefined ? {
            reader: new StreamMessageReader(server.stdout), writer: new StreamMessageWriter(server.stdin),
        } : protocol === 'ws' ? await connectToWebSocketServer(port) : await (async () => {
            const streams = await connectToServer(port);
            t.after(() => {
                (streams.writer as import('node:net').Socket).destroy();
            });
            return { reader: new StreamMessageReader(streams.reader), writer: new StreamMessageWriter(streams.writer) };
        })();
        const connection = createMessageConnection(transports.reader, transports.writer);
        t.after(() => connection.dispose());
        connection.listen();
        const result = await connection.sendRequest<{ capabilities: object }>('initialize', {
            processId: process.pid, rootUri: null, capabilities: {},
        });
        assert.ok(result.capabilities);
        await connection.sendNotification('initialized', {});
        assert.equal(await connection.sendRequest('shutdown'), null);
        await connection.sendNotification('exit');
        assert.deepEqual(await exited, [0, null], logs);
    });
}
