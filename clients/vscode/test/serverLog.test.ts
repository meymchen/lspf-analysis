import assert from 'node:assert/strict';
import { appendFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout } from 'node:timers/promises';
import test from 'node:test';
import { followServerLog } from '../src/serverLog.js';

test('Output receives startup and subsequent logs with split UTF-8, then stops on disposal', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'lspf-log-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const filename = join(directory, 'server.log');
  writeFileSync(filename, 'DEBUG startup\n');
  let output = '';
  const follower = followServerLog(filename, {
    append: (value) => {
      output += value;
    },
  });
  t.after(() => follower.dispose());
  assert.equal(output, 'DEBUG startup\n');
  const message = Buffer.from('中文日志\n');
  appendFileSync(filename, message.subarray(0, 2));
  await setTimeout(150);
  assert.equal(output, 'DEBUG startup\n');
  appendFileSync(filename, message.subarray(2));
  await setTimeout(150);
  assert.equal(output, 'DEBUG startup\n中文日志\n');
  writeFileSync(filename, 'restart\n');
  await setTimeout(150);
  assert.equal(output, 'DEBUG startup\n中文日志\nrestart\n');
  follower.dispose();
  appendFileSync(filename, 'after disposal\n');
  await setTimeout(150);
  assert.equal(output, 'DEBUG startup\n中文日志\nrestart\n');
});

test('Output can follow a log that does not exist yet', async (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'lspf-log-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const filename = join(directory, 'server.log');
  let output = '';
  const follower = followServerLog(filename, {
    append: (value) => {
      output += value;
    },
  });
  t.after(() => follower.dispose());
  writeFileSync(filename, 'late startup\n');
  await setTimeout(150);
  assert.equal(output, 'late startup\n');
});
