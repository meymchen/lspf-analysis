import { closeSync, fstatSync, openSync, readSync } from 'node:fs';
import { createInterface } from 'node:readline';
import type { Readable } from 'node:stream';
import { StringDecoder } from 'node:string_decoder';

type Level = 'trace' | 'debug' | 'info' | 'warn' | 'error';

/** The part of a VS Code `LogOutputChannel` the server's log is written to. */
export type ServerLog = Record<Level, (message: string) => void>;

/**
 * A line from the server's `tracing` subscriber: a timestamp, the level
 * padded to five columns, then the target and message.
 */
const TRACING_LINE = /^\S+\s+(TRACE|DEBUG|INFO|WARN|ERROR)\s+(.*)$/;

/**
 * Writes one server log line at the level the server gave it.
 *
 * The channel adds its own timestamp and level, so the server's are dropped.
 * A line without them, such as a fatal `eprintln!`, goes in at `fallback`.
 */
export function logServerLine(line: string, log: ServerLog, fallback: Level): void {
  if (!line) {
    return;
  }
  const match = TRACING_LINE.exec(line);
  if (match) {
    log[match[1].toLowerCase() as Level](match[2]);
  } else {
    log[fallback](line);
  }
}

/** Forwards a server output stream to `log` a line at a time. */
export function forwardServerLog(input: Readable, log: ServerLog, fallback: Level): void {
  createInterface({ input, crlfDelay: Infinity, terminal: false, historySize: 0 }).on(
    'line',
    (line) => logServerLine(line, log, fallback),
  );
}

/** Replay startup logs, then follow the externally debugged server into Output. */
export function followServerLog(filename: string, log: ServerLog): { dispose(): void } {
  let position = 0;
  let decoder = new StringDecoder('utf8');
  // Text after the last newline, held back until the rest of its line is read.
  let pending = '';
  let lastError: string | undefined;
  const buffer = Buffer.alloc(64 * 1024);
  function poll(): void {
    let fd: number | undefined;
    try {
      fd = openSync(filename, 'r');
      const size = fstatSync(fd).size;
      if (size < position) {
        position = 0;
        decoder = new StringDecoder('utf8');
        pending = '';
      }
      // Bound each read so a busy server cannot monopolize the extension host.
      const length = readSync(fd, buffer, 0, Math.min(buffer.length, size - position), position);
      position += length;
      const lines = (pending + decoder.write(buffer.subarray(0, length))).split(/\r?\n/);
      pending = lines.pop() ?? '';
      for (const line of lines) {
        logServerLine(line, log, 'info');
      }
      lastError = undefined;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') {
        const message = String(error);
        if (message !== lastError) log.error(`Cannot read server log: ${message}`);
        lastError = message;
      }
    } finally {
      if (fd !== undefined) closeSync(fd);
    }
  }
  poll();
  const timer = setInterval(poll, 100);
  timer.unref();
  return { dispose: () => clearInterval(timer) };
}
