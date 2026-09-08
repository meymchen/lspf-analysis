import * as assert from 'node:assert/strict';
import * as net from 'node:net';
import { once } from 'node:events';
import test from 'node:test';
import { WebSocketServer } from 'ws';

import {
  DEBUG_PORT_VARIABLE,
  DEBUG_TRANSPORT_VARIABLE,
  connectToServer,
  connectToWebSocketServer,
  debugServerPort,
  debugServerTransport,
} from '../src/debug.js';

test('debug transport defaults to TCP and accepts WebSocket', () => {
  assert.equal(debugServerTransport({}), 'tcp');
  assert.equal(debugServerTransport({ [DEBUG_TRANSPORT_VARIABLE]: 'tcp' }), 'tcp');
  assert.equal(debugServerTransport({ [DEBUG_TRANSPORT_VARIABLE]: ' ws ' }), 'ws');
  assert.throws(
    () => debugServerTransport({ [DEBUG_TRANSPORT_VARIABLE]: 'udp' }),
    /must be tcp or ws/,
  );
});

test('legacy TCP port works and the generic port takes precedence', () => {
  assert.equal(debugServerPort({ LSPF_ANALYSIS_DEBUG_TCP: '9257' }), 9257);
  assert.equal(
    debugServerPort({ LSPF_ANALYSIS_DEBUG_TCP: '9257', [DEBUG_PORT_VARIABLE]: '9258' }),
    9258,
  );
});

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

test(
  'WebSocket retries startup, exchanges JSON text frames, and closes on disposal',
  { timeout: 5000 },
  async (t) => {
    const port = await freePort();
    const connecting = connectToWebSocketServer(port, { timeout: 2000, interval: 25 });
    await new Promise((resolve) => setTimeout(resolve, 150));
    const server = new WebSocketServer({ host: '127.0.0.1', port });
    t.after(() => {
      for (const client of server.clients) client.terminate();
      return new Promise<void>((resolve) => server.close(() => resolve()));
    });
    const accepted = once(server, 'connection');
    const transport = await connecting;
    t.after(() => {
      transport.reader.dispose();
      transport.writer.dispose();
    });
    const [socket] = await accepted;
    const closed = once(socket, 'close');
    const request = once(socket, 'message');
    const response = new Promise((resolve) => transport.reader.listen(resolve));
    const initialize = { jsonrpc: '2.0', id: 1, method: 'initialize', params: {} };
    await transport.writer.write(initialize);
    const [data, binary] = await request;
    assert.equal(binary, false);
    assert.equal(JSON.parse(data.toString()).method, 'initialize');
    socket.send(JSON.stringify({ jsonrpc: '2.0', id: 1, result: { name: '中文' } }));
    assert.deepEqual(await response, { jsonrpc: '2.0', id: 1, result: { name: '中文' } });
    transport.writer.dispose();
    await closed;
  },
);

test('WebSocket failure names the required server mode', { timeout: 5000 }, async () => {
  const port = await freePort();
  await assert.rejects(
    connectToWebSocketServer(port, { timeout: 150, interval: 25 }),
    /serve --ws/,
  );
});

test(
  'WebSocket handshake timeout closes a peer that never upgrades',
  { timeout: 5000 },
  async (t) => {
    const sockets = new Set<net.Socket>();
    const server = net.createServer((socket) => {
      sockets.add(socket);
      socket.on('close', () => sockets.delete(socket));
      socket.resume();
    });
    t.after(() => {
      for (const socket of sockets) socket.destroy();
      return new Promise<void>((resolve) => server.close(() => resolve()));
    });
    server.listen(0, '127.0.0.1');
    await once(server, 'listening');
    const port = (server.address() as net.AddressInfo).port;
    const accepted = once(server, 'connection');
    const connecting = connectToWebSocketServer(port, { timeout: 150, interval: 25 });
    const [socket] = await accepted;
    const closed = once(socket, 'close');
    await assert.rejects(connecting, /serve --ws/);
    await closed;
  },
);
