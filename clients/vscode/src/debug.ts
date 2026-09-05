import * as net from 'node:net';

import type { StreamInfo } from 'vscode-languageclient/node';

/**
 * Set to a port by the "Debug language server" launch configuration.
 *
 * When it is present the extension talks to a server VS Code has already
 * started under a debugger, rather than spawning one of its own, so both
 * halves can be stepped through in the same session. Nothing sets it in a
 * released extension.
 */
export const DEBUG_PORT_VARIABLE = 'LSPF_ANALYSIS_DEBUG_TCP';

/** Reads the port to attach to, or `undefined` outside a debug session. */
export function debugServerPort(env: NodeJS.ProcessEnv): number | undefined {
    const raw = env[DEBUG_PORT_VARIABLE]?.trim();
    if (!raw) {
        return undefined;
    }
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
        throw new Error(`${DEBUG_PORT_VARIABLE} is ${raw}, which is not a port number`);
    }
    return port;
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
 * The two processes start in parallel, and the debugger delays the server
 * enough that the first connection usually loses the race, so keep trying
 * for as long as a person would wait before assuming it never started.
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
                        'Start the "Debug language server" configuration, which runs it ' +
                        `with serve --tcp ${host}:${port}. (${String(error)})`,
                );
            }
            await sleep(interval);
        }
    }
}
