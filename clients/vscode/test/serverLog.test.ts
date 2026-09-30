import assert from 'node:assert/strict';
import { appendFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { PassThrough } from 'node:stream';
import { setTimeout } from 'node:timers/promises';
import test from 'node:test';
import {
  followServerLog,
  forwardServerLog,
  logServerLine,
  type ServerLog,
} from '../src/serverLog.js';

/** A log that records `level message` for each entry. */
function recordingLog(): { log: ServerLog; entries: string[] } {
  const entries: string[] = [];
  const at = (level: string) => (message: string) => entries.push(`${level} ${message}`);
  return {
    log: {
      trace: at('trace'),
      debug: at('debug'),
      info: at('info'),
      warn: at('warn'),
      error: at('error'),
    },
    entries,
  };
}

test('A tracing line is logged at its own level without its timestamp', () => {
  const { log, entries } = recordingLog();
  logServerLine('2026-09-30T00:44:58.570318Z DEBUG lspf_analysis: started', log, 'error');
  logServerLine('2026-09-30T00:44:58.570318Z  INFO lspf_analysis: ready', log, 'error');
  logServerLine('2026-09-30T00:44:58.570318Z  WARN lspf_analysis: slow', log, 'error');
  assert.deepEqual(entries, [
    'debug lspf_analysis: started',
    'info lspf_analysis: ready',
    'warn lspf_analysis: slow',
  ]);
});

test('A line without a level is logged at the fallback, and a blank one not at all', () => {
  const { log, entries } = recordingLog();
  logServerLine('lspf-analysis: cannot open log file x', log, 'error');
  logServerLine('', log, 'error');
  assert.deepEqual(entries, ['error lspf-analysis: cannot open log file x']);
});

test('A stream is forwarded a line at a time', async () => {
  const { log, entries } = recordingLog();
  const input = new PassThrough();
  forwardServerLog(input, log, 'error');
  input.write('2026-09-30T00:44:58Z  INFO lspf_analysis: re');
  input.write('ady\r\npanicked\n');
  input.end();
  await setTimeout(10);
  assert.deepEqual(entries, ['info lspf_analysis: ready', 'error panicked']);
});

test('Output receives startup and subsequent logs with split UTF-8, then stops on disposal', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'lspf-log-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const filename = join(directory, 'server.log');
  writeFileSync(filename, '2026-09-30T00:44:58Z DEBUG startup\n');
  const { log, entries } = recordingLog();
  const follower = followServerLog(filename, log);
  t.after(() => follower.dispose());
  assert.deepEqual(entries, ['debug startup']);
  const message = Buffer.from('中文日志\n');
  appendFileSync(filename, message.subarray(0, 2));
  await setTimeout(150);
  assert.deepEqual(entries, ['debug startup']);
  appendFileSync(filename, message.subarray(2));
  await setTimeout(150);
  assert.deepEqual(entries, ['debug startup', 'info 中文日志']);
  writeFileSync(filename, 'restart\n');
  await setTimeout(150);
  assert.deepEqual(entries, ['debug startup', 'info 中文日志', 'info restart']);
  follower.dispose();
  appendFileSync(filename, 'after disposal\n');
  await setTimeout(150);
  assert.deepEqual(entries, ['debug startup', 'info 中文日志', 'info restart']);
});

test('Output holds a partial line until its newline arrives', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'lspf-log-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const filename = join(directory, 'server.log');
  writeFileSync(filename, '2026-09-30T00:44:58Z  WARN hal');
  const { log, entries } = recordingLog();
  const follower = followServerLog(filename, log);
  t.after(() => follower.dispose());
  assert.deepEqual(entries, []);
  appendFileSync(filename, 'f a line\n');
  await setTimeout(150);
  assert.deepEqual(entries, ['warn half a line']);
});

test('Output can follow a log that does not exist yet', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'lspf-log-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const filename = join(directory, 'server.log');
  const { log, entries } = recordingLog();
  const follower = followServerLog(filename, log);
  t.after(() => follower.dispose());
  writeFileSync(filename, 'late startup\n');
  await setTimeout(150);
  assert.deepEqual(entries, ['info late startup']);
});
