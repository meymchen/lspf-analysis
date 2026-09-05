import * as assert from 'node:assert/strict';
import * as net from 'node:net';
import test from 'node:test';

import { DEBUG_PORT_VARIABLE, connectToServer, debugServerPort } from '../src/debug.js';

test('no debug port means the extension spawns its own server', () => {
    assert.equal(debugServerPort({}), undefined);
    assert.equal(debugServerPort({ [DEBUG_PORT_VARIABLE]: '' }), undefined);
    assert.equal(debugServerPort({ [DEBUG_PORT_VARIABLE]: '  ' }), undefined);
});

test('a debug port is read as a number', () => {
    assert.equal(debugServerPort({ [DEBUG_PORT_VARIABLE]: '9257' }), 9257);
    assert.equal(debugServerPort({ [DEBUG_PORT_VARIABLE]: ' 9257 ' }), 9257);
});

test('a port that is not one is rejected rather than guessed at', () => {
    for (const raw of ['nonsense', '0', '-1', '65536', '9257.5']) {
        assert.throws(
            () => debugServerPort({ [DEBUG_PORT_VARIABLE]: raw }),
            new RegExp(DEBUG_PORT_VARIABLE),
        );
    }
});

/** Takes a port, then frees it, so a test can decide when to listen on it. */
async function freePort(): Promise<number> {
    const probe = net.createServer();
    const port = await new Promise<number>((resolve) => {
        probe.listen(0, '127.0.0.1', () => resolve((probe.address() as net.AddressInfo).port));
    });
    await new Promise((resolve) => probe.close(resolve));
    return port;
}

test('connecting waits for a server that starts late', async () => {
    const port = await freePort();
    // Start connecting into nothing, which is the race a server held up by a
    // debugger loses every time.
    const connecting = connectToServer(port, { timeout: 5000, interval: 25 });

    const server = net.createServer();
    await new Promise((resolve) => setTimeout(resolve, 150));
    await new Promise<void>((resolve) => server.listen(port, '127.0.0.1', resolve));

    const stream = await connecting;
    assert.ok(stream.reader);
    assert.ok(stream.writer);
    (stream.reader as net.Socket).destroy();
    await new Promise((resolve) => server.close(resolve));
});

test('connecting gives up with an actionable message', async () => {
    const port = await freePort();
    await assert.rejects(connectToServer(port, { timeout: 150, interval: 25 }), /serve --tcp/);
});
