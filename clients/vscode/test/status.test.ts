import * as assert from 'node:assert/strict';
import test from 'node:test';

import { isFileHealth, renderStatus, type FileHealth } from '../src/status.js';

const healthy: FileHealth = {
    uri: 'file:///a.rs',
    quality: 87.4,
    grade: 'excellent',
    functions: 3,
    below: 0,
    worst: { name: 'add', quality: 80.2, grade: 'excellent', line: 12 },
};

test('the bar shows the band and the rounded score', () => {
    const rendered = renderStatus(healthy);
    assert.equal(rendered.text, '$(pass) 87%');
    assert.equal(rendered.warning, false);
});

test('a bad file is coloured and iconed differently', () => {
    assert.equal(renderStatus({ ...healthy, grade: 'poor' }).text, '$(error) 87%');
    assert.equal(renderStatus({ ...healthy, grade: 'poor' }).warning, true);
    assert.equal(renderStatus({ ...healthy, grade: 'fair' }).text, '$(warning) 87%');
    assert.equal(renderStatus({ ...healthy, grade: 'fair' }).warning, true);
    assert.equal(renderStatus({ ...healthy, grade: 'good' }).text, '$(check) 87%');
    assert.equal(renderStatus({ ...healthy, grade: 'good' }).warning, false);
});

test('the tooltip carries what the bar has no room for', () => {
    const { tooltip } = renderStatus(healthy);
    assert.match(tooltip, /file quality \*\*87%\*\* \(excellent\)/);
    assert.match(tooltip, /3 functions analyzed, 0 below/);
    assert.match(tooltip, /Worst: `add` at 80% \(excellent\), line 12/);
});

test('one function is not pluralized', () => {
    const { tooltip } = renderStatus({ ...healthy, functions: 1 });
    assert.match(tooltip, /1 function analyzed/);
});

test('a file with no functions has no worst one to name', () => {
    const { tooltip } = renderStatus({
        ...healthy,
        functions: 0,
        quality: 100,
        worst: null,
    });
    assert.match(tooltip, /0 functions analyzed/);
    assert.doesNotMatch(tooltip, /Worst/);
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

test('a payload without the optional worst function is still usable', () => {
    const { worst, ...withoutWorst } = healthy;
    void worst;
    assert.equal(isFileHealth(withoutWorst), true);
});
