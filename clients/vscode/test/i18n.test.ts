import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import test from 'node:test';

import { functionTooltip, type FunctionDetail } from '../src/functions.js';
import { fill, gradeLabel, measureLabel, pillarLabel, t, useTranslator } from '../src/i18n.js';
import { renderStatus, type FileHealth } from '../src/status.js';

/** The extension root, from wherever the compiled test is being run. */
const root = path.resolve(__dirname, '../..');

const read = (file: string): Record<string, string> =>
  JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));

/**
 * Every string the sources pass to `t`.
 *
 * Extracted rather than listed, so a string added without a translation is a
 * failing test instead of a sentence that stays English for one reader.
 */
function translatedSources(): Set<string> {
  const sources = new Set<string>();
  const directory = path.join(root, 'src');
  for (const file of fs.readdirSync(directory)) {
    if (!file.endsWith('.ts')) {
      continue;
    }
    const code = fs.readFileSync(path.join(directory, file), 'utf8');
    // `t('a' + 'b', …)`, but not `format(`, `assert(` or a property call.
    const calls = /(?<![A-Za-z0-9_.$])t\(\s*((?:'(?:[^'\\]|\\.)*'\s*\+?\s*)+)/g;
    for (const call of code.matchAll(calls)) {
      const literals = [...call[1]!.matchAll(/'((?:[^'\\]|\\.)*)'/g)].map((part) => part[1]);
      sources.add(literals.join(''));
    }
  }
  return sources;
}

const health: FileHealth = {
  uri: 'file:///a.rs',
  quality: 40,
  grade: 'fair',
  functions: 2,
  below: 1,
  bands: { excellent: 0, good: 1, fair: 0, poor: 1 },
  worst: [
    {
      name: 'tangled',
      quality: 12,
      grade: 'poor',
      line: 4,
      weakestPillar: 'control flow',
      weakestMetric: 'cognitive complexity',
    },
  ],
};

const detail: FunctionDetail = {
  name: 'tangled',
  startLine: 4,
  endLine: 40,
  quality: 12,
  grade: 'poor',
  weakestPillar: 'control flow',
  weakestMetric: 'cognitive complexity',
  pillars: [],
};

test('a placeholder is filled by its index', () => {
  assert.equal(fill('{0} of {1}', ['one', 'two']), 'one of two');
  assert.equal(fill('{1} then {0}', ['a', 'b']), 'b then a', 'a translation may reorder');
  assert.equal(fill('{0} and {1}', ['one']), 'one and {1}', 'a missing one stays visible');
  assert.equal(fill('nothing', []), 'nothing');
});

test('without a translator every string is the English it was written in', () => {
  assert.equal(t('control flow'), 'control flow');
  assert.equal(t('Go to line {0}', 12), 'Go to line 12');
});

test('a name the server invented is shown as it arrived', () => {
  assert.equal(pillarLabel('coupling'), 'coupling', 'a pillar from a newer server');
  assert.equal(measureLabel('fan-out'), 'fan-out');
  assert.equal(gradeLabel('sublime'), 'sublime');
});

test('every rendered string goes through the translator', (context) => {
  // The point of the test is that nothing is hardcoded past `t`: with a
  // translator that marks what it touches, no unmarked prose is left.
  context.after(() => useTranslator(undefined));
  useTranslator((message, ...args) => `«${fill(message, args)}»`);

  const tooltip = renderStatus(health).tooltip;
  for (const marked of [
    '«Worth opening first»',
    '«Problems»',
    '«Settings»',
    '«Restart server»',
    '«Go to line 4»',
    '«2 functions, 1 below the warning threshold.»',
    // The vocabulary the server sent, translated where it is drawn.
    '«control flow»',
    '«cognitive complexity»',
    '«poor»',
    '«band»',
  ]) {
    assert.ok(tooltip.includes(marked), `${marked} is missing from:\n${tooltip}`);
  }

  const rendered = functionTooltip(detail);
  for (const marked of ['«quality»', '«poor»', '«Lines 4–40.»', '«control flow»']) {
    assert.ok(rendered.includes(marked), `${marked} is missing from:\n${rendered}`);
  }
});

test('the Simplified Chinese bundle covers every translated string', () => {
  const bundle = read('l10n/bundle.l10n.zh-cn.json');
  const missing = [...translatedSources()].filter((source) => !(source in bundle));
  assert.deepEqual(missing, [], 'strings with no Simplified Chinese');
});

test('the Simplified Chinese bundle has nothing left over', () => {
  const bundle = read('l10n/bundle.l10n.zh-cn.json');
  const sources = translatedSources();
  const stale = Object.keys(bundle).filter((key) => !sources.has(key));
  assert.deepEqual(stale, [], 'translations of strings no longer rendered');
});

test('a translated string keeps the placeholders it was given', () => {
  const bundle = read('l10n/bundle.l10n.zh-cn.json');
  const placeholders = (value: string) =>
    [...value.matchAll(/\{(\d+)\}/g)].map((match) => match[1]).sort();
  for (const [source, translation] of Object.entries(bundle)) {
    assert.deepEqual(
      placeholders(translation),
      placeholders(source),
      `${source} loses or invents a placeholder`,
    );
  }
});

test('the manifest bundles name the same keys', () => {
  const english = read('package.nls.json');
  const chinese = read('package.nls.zh-cn.json');
  assert.deepEqual(Object.keys(chinese).sort(), Object.keys(english).sort());
});

test('every %placeholder% in the manifest is declared', () => {
  const manifest = fs.readFileSync(path.join(root, 'package.json'), 'utf8');
  const english = read('package.nls.json');
  const used = new Set([...manifest.matchAll(/"%([^%"]+)%"/g)].map((match) => match[1]!));
  assert.notEqual(used.size, 0, 'the manifest is localized at all');
  for (const key of used) {
    assert.ok(key in english, `${key} is used but not declared`);
  }
  for (const key of Object.keys(english)) {
    assert.ok(used.has(key), `${key} is declared but not used`);
  }
});
