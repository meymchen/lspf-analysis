import * as assert from 'node:assert/strict';
import test from 'node:test';

import {
  GO_TO_SOURCE_COMMAND,
  bar,
  icon,
  isFileHealth,
  renderStatus,
  type FileHealth,
} from '../src/status.js';

const healthy: FileHealth = {
  uri: 'file:///a.rs',
  quality: 87.4,
  grade: 'excellent',
  functions: 4,
  below: 0,
  bands: { excellent: 3, good: 1, fair: 0, poor: 0 },
  worst: [
    {
      name: 'add',
      quality: 80.2,
      grade: 'excellent',
      line: 12,
      weakestPillar: 'interface',
      weakestMetric: 'parameters',
    },
  ],
};

test('a bar fills in proportion to the score', () => {
  assert.equal(bar(0), '░░░░░░░░░░');
  assert.equal(bar(100), '██████████');
  assert.equal(bar(50), '█████░░░░░');
  assert.equal(bar(-20), '░░░░░░░░░░', 'a nonsense score does not overflow the bar');
  assert.equal(bar(150), '██████████');
});

test('a bar resolves a fraction of a cell', () => {
  // Eight steps within every cell, so two scores a percent apart do not
  // draw the same bar.
  assert.equal(bar(6.25), '▋░░░░░░░░░');
  assert.equal(bar(55), '█████▌░░░░');
  assert.notEqual(bar(51), bar(54));
  // However it divides, a bar is always the same width.
  for (let step = 0; step <= 200; step += 1) {
    assert.equal([...bar(step / 2)].length, 10, `bar(${step / 2})`);
  }
  assert.equal([...bar(Number.NaN)].length, 10, 'a NaN score still draws a bar');
});

test('the bar shows the band and the rounded score', () => {
  const rendered = renderStatus(healthy);
  assert.equal(rendered.text, '$(pass) 87%');
  assert.equal(rendered.warning, false);
});

test('functions below the threshold are counted in the bar itself', () => {
  const rendered = renderStatus({ ...healthy, below: 3 });
  assert.match(rendered.text, /\$\(alert\)3$/);
});

test('a bad file is coloured and iconed differently', () => {
  for (const [grade, expected, warning] of [
    ['poor', '$(error)', true],
    ['fair', '$(warning)', true],
    ['good', '$(check)', false],
  ] as const) {
    const rendered = renderStatus({ ...healthy, grade });
    assert.equal(icon(grade), expected);
    assert.ok(rendered.text.startsWith(expected), rendered.text);
    assert.equal(rendered.warning, warning);
  }
});

test('the tooltip carries the spread across bands', () => {
  const { tooltip } = renderStatus(healthy);
  assert.match(tooltip, /\*\*87%\*\*/);
  assert.match(tooltip, /4 functions, 0 below the warning threshold/);
  assert.match(tooltip, /\| \$\(pass\) \| excellent \| 3 \|/);
  assert.match(tooltip, /\| \$\(error\) \| poor \| 0 \|/);
});

test('each worst function links to its line', () => {
  const { tooltip } = renderStatus(healthy);
  assert.match(tooltip, /`add`/);
  assert.match(tooltip, /80%/);
  assert.match(tooltip, /weakest interface \(parameters\)/);
  const encoded = encodeURIComponent(JSON.stringify(['file:///a.rs', 12]));
  assert.ok(
    tooltip.includes(`command:${GO_TO_SOURCE_COMMAND}?${encoded}`),
    `no go-to link in:\n${tooltip}`,
  );
});

test('the tooltip offers the actions the status bar cannot', () => {
  const { tooltip } = renderStatus(healthy);
  assert.match(tooltip, /command:workbench\.actions\.view\.problems/);
  assert.match(tooltip, /command:workbench\.action\.openSettings/);
  assert.match(tooltip, /command:lspfAnalysis\.restartServer/);
});

test('no bar is wrapped in a code span', () => {
  // VS Code draws inline code with a background and padding, which would
  // put a gap on either side of every bar.
  const { tooltip } = renderStatus(healthy);
  assert.doesNotMatch(tooltip, /`[█▉▊▋▌▍▎▏░▓▒]/, tooltip);
  assert.doesNotMatch(tooltip, /[█▉▊▋▌▍▎▏░▓▒]`/, tooltip);
});

test('one function is not pluralized', () => {
  const { tooltip } = renderStatus({ ...healthy, functions: 1 });
  assert.match(tooltip, /1 function,/);
});

test('a file with no functions says so instead of showing empty bands', () => {
  const { tooltip } = renderStatus({
    ...healthy,
    functions: 0,
    quality: 100,
    bands: { excellent: 0, good: 0, fair: 0, poor: 0 },
    worst: [],
  });
  assert.match(tooltip, /No functions to analyze/);
  assert.doesNotMatch(tooltip, /Worth opening first/);
});

test('a payload from an unrecognizable server is ignored', () => {
  assert.equal(isFileHealth(healthy), true);
  assert.equal(isFileHealth(undefined), false);
  assert.equal(isFileHealth(null), false);
  assert.equal(isFileHealth('nonsense'), false);
  assert.equal(isFileHealth({ ...healthy, quality: 'high' }), false);
  assert.equal(isFileHealth({ ...healthy, quality: Number.NaN }), false);
  assert.equal(isFileHealth({ uri: 'file:///a.rs' }), false);
});

test('a payload from the earlier three-pillar server is rejected', () => {
  // It sent `worst` as a single object and had no bands at all; rendering
  // it would produce a tooltip full of `undefined`.
  const old = {
    uri: 'file:///a.rs',
    quality: 87.4,
    grade: 'excellent',
    functions: 4,
    below: 0,
    worst: { name: 'add', quality: 80.2, grade: 'excellent', line: 12 },
  };
  assert.equal(isFileHealth(old), false);
});
