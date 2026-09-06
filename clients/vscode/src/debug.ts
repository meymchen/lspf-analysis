import * as net from 'node:net';
import WebSocket from 'ws';

import type { MessageTransports, StreamInfo } from 'vscode-languageclient/node';

/**
 * Set to a port by the TCP and WebSocket extension host configurations.
 *
 * When it is present the extension talks to a server VS Code has already
 * started in a separate debug session, rather than spawning one of its own.
 * Both sessions can be stepped through independently. Nothing sets it in a
 * released extension.
 */
export const DEBUG_PORT_VARIABLE = 'LSPF_ANALYSIS_DEBUG_PORT';
export const DEBUG_TRANSPORT_VARIABLE = 'LSPF_ANALYSIS_DEBUG_TRANSPORT';

export function debugServerTransport(env: NodeJS.ProcessEnv): 'tcp' | 'ws' {
    const transport = env[DEBUG_TRANSPORT_VARIABLE]?.trim() || 'tcp';
    if (transport !== 'tcp' && transport !== 'ws') {
        throw new Error(`${DEBUG_TRANSPORT_VARIABLE} must be tcp or ws, got ${transport}`);
    }
    return transport;
}

/** Reads the port to attach to, or `undefined` outside a debug session. */
export function debugServerPort(env: NodeJS.ProcessEnv): number | undefined {
    // Keep manually configured TCP debug sessions working.
    const variable = env[DEBUG_PORT_VARIABLE] === undefined ? 'LSPF_ANALYSIS_DEBUG_TCP' : DEBUG_PORT_VARIABLE;
    const raw = env[variable]?.trim();
    if (!raw) {
        return undefined;
    }
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
        throw new Error(`${variable} is ${raw}, which is not a port number`);
    }
    return port;
}

/** Connect JSON-RPC messages directly to WebSocket text frames. */
export async function connectToWebSocketServer(
    port: number,
    { host = '127.0.0.1', timeout = 20000, interval = 200 } = {},
): Promise<MessageTransports> {
    const { WebSocketMessageReader, WebSocketMessageWriter } = await import('vscode-ws-jsonrpc');
    const deadline = Date.now() + timeout;
    for (;;) {
        try {
            return await new Promise<MessageTransports>((resolve, reject) => {
                const socket = new WebSocket(`ws://${host}:${port}`, {
                    handshakeTimeout: Math.max(1, deadline - Date.now()),
                });
                // Keep an error handler through termination, which can emit another error.
                socket.on('error', reject);
                socket.once('open', () => {
                    const adapter = {
                        send: (content: string) => socket.send(content),
                        onMessage: (callback: (data: string) => void) =>
                            socket.on('message', data => callback(data.toString())),
                        onError: (callback: (error: Error) => void) => socket.on('error', callback),
                        onClose: (callback: (code: number, reason: string) => void) =>
                            socket.on('close', (code, reason) => callback(code, reason.toString())),
                        dispose: () => socket.terminate(),
                    };
                    class Writer extends WebSocketMessageWriter {
                        override end(): void { socket.close(); }
                        override dispose(): void {
                            super.dispose();
                            socket.terminate();
                        }
                    }
                    resolve({ reader: new WebSocketMessageReader(adapter), writer: new Writer(adapter) });
                });
            });
        } catch (error) {
            if (Date.now() >= deadline) {
                throw new Error(
                    `no lspf-analysis WebSocket server on ${host}:${port} after ${timeout}ms. ` +
                    'Start the "websocket" debug configuration, ' +
                    `which runs serve --ws ${host}:${port}. (${String(error)})`,
                );
            }
            await sleep(Math.min(interval, deadline - Date.now()));
        }
    }
}

function openSocket(port: number, host: string): Promise<net.Socket> {
    return new Promise((resolve, reject) => {
        const socket = net.createConnection({ port, host });
        const failed = (error: Error) => {
            socket.destroy();
            reject(error);
        };
        socket.once('error', failed);
        socket.once('connect', () => {
            socket.removeListener('error', failed);
            resolve(socket);
        });
    });
}

const sleep = (milliseconds: number) =>
    new Promise((resolve) => setTimeout(resolve, milliseconds));

/**
 * Connects to a server listening on `port`, retrying until it is up.
 *
 * The compound launch starts the server and development host together.
 * Retries allow the host to connect to a server that
 * is still starting or paused in the debugger.
 */
export async function connectToServer(
    port: number,
    { host = '127.0.0.1', timeout = 20000, interval = 200 } = {},
): Promise<StreamInfo> {
    const deadline = Date.now() + timeout;
    for (;;) {
        try {
            const socket = await openSocket(port, host);
            return { reader: socket, writer: socket };
        } catch (error) {
            if (Date.now() >= deadline) {
                throw new Error(
                    `no lspf-analysis server on ${host}:${port} after ${timeout}ms. ` +
                        'Start the "tcp" debug configuration, which runs it ' +
                        `with serve --tcp ${host}:${port}. (${String(error)})`,
                );
            }
            await sleep(interval);
        }
    }
}
