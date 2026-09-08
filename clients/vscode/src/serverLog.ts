import { closeSync, fstatSync, openSync, readSync } from 'node:fs';
import { StringDecoder } from 'node:string_decoder';

/** Replay startup logs, then follow the externally debugged server into Output. */
export function followServerLog(
  filename: string,
  output: { append(value: string): void },
): { dispose(): void } {
  let position = 0;
  let decoder = new StringDecoder('utf8');
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
      }
      // Bound each read so a busy server cannot monopolize the extension host.
      const length = readSync(fd, buffer, 0, Math.min(buffer.length, size - position), position);
      position += length;
      output.append(decoder.write(buffer.subarray(0, length)));
      lastError = undefined;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') {
        const message = String(error);
        if (message !== lastError) output.append(`Cannot read server log: ${message}\n`);
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
