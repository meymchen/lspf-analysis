import * as assert from 'node:assert/strict';
import test from 'node:test';

import {
    badge,
    functionDescription,
    functionTooltip,
    gradeOf,
    isFunctionHealth,
    measureDescription,
    measureTooltip,
    pillarDescription,
    sortFunctions,
    type FunctionDetail,
    type FunctionHealth,
} from '../src/functions.js';

function detail(overrides: Partial<FunctionDetail> = {}): FunctionDetail {
    return {
        name: 'add',
        startLine: 12,
        endLine: 18,
        quality: 87.4,
        grade: 'excellent',
        weakestPillar: 'interface',
        weakestMetric: 'parameters',
        pillars: [
            {
                name: 'control flow',
                score: 96.2,
                measures: [
                    { name: 'cognitive complexity', value: 3, threshold: 15, score: 96.2 },
                    { name: 'cyclomatic complexity', value: 2, threshold: 10, score: 96.2 },
                ],
            },
            {
                name: 'interface',
                score: 64,
                measures: [{ name: 'parameters', value: 3, threshold: 4, score: 64 }],
            },
        ],
        ...overrides,
    };
}

const health: FunctionHealth = { uri: 'file:///a.rs', functions: [detail()] };

test('the tree orders the worst function first by default', () => {
    const functions = [
        detail({ name: 'fine', quality: 90, startLine: 1 }),
        detail({ name: 'awful', quality: 12, startLine: 40 }),
        detail({ name: 'middling', quality: 55, startLine: 20 }),
    ];
    assert.deepEqual(
        sortFunctions(functions, 'quality').map((f) => f.name),
        ['awful', 'middling', 'fine'],
    );
    assert.deepEqual(
        sortFunctions(functions, 'position').map((f) => f.name),
        ['fine', 'middling', 'awful'],
    );
});

test('sorting leaves the answer from the server alone', () => {
    const functions = [detail({ name: 'a', quality: 90 }), detail({ name: 'b', quality: 10 })];
    sortFunctions(functions, 'quality');
    assert.deepEqual(
        functions.map((f) => f.name),
        ['a', 'b'],
        'the caller keeps source order to key rows by',
    );
});

test('functions scoring the same keep a stable order between refreshes', () => {
    const functions = [
        detail({ name: 'second', quality: 50, startLine: 30 }),
        detail({ name: 'first', quality: 50, startLine: 10 }),
    ];
    assert.deepEqual(
        sortFunctions(functions, 'quality').map((f) => f.name),
        ['first', 'second'],
    );
});

test('a row says what it scored and what dragged it down', () => {
    assert.equal(functionDescription(detail()), '87%  ·  interface');
    assert.equal(pillarDescription(detail().pillars[1]!), '64%');
    assert.equal(
        measureDescription({ name: 'parameters', value: 3, threshold: 4, score: 64 }),
        '3 / 4  ·  64%',
    );
});

test('a tooltip names the band, the span and the weakest measure', () => {
    const tooltip = functionTooltip(detail());
    assert.match(tooltip, /\$\(pass\)/);
    assert.match(tooltip, /`add`/);
    assert.match(tooltip, /87%/);
    assert.match(tooltip, /Lines 12–18\./);
    assert.match(tooltip, /Weakest: \*\*interface\*\* — parameters\./);
});

test('a function with no measure to blame says nothing about one', () => {
    const tooltip = functionTooltip(detail({ weakestMetric: '' }));
    assert.doesNotMatch(tooltip, /Weakest/);
});

test('a measure tooltip says what it was judged against', () => {
    const tooltip = measureTooltip({ name: 'statements', value: 41, threshold: 30, score: 34.9 });
    assert.match(tooltip, /\*\*statements\*\*/);
    assert.match(tooltip, /Measured 41 against a threshold of 30/);
    assert.match(tooltip, /35%/, 'the score is rounded, as everywhere else');
});

test('a band maps to an icon and a chart colour', () => {
    assert.deepEqual(badge('excellent'), { id: 'pass', color: 'charts.green' });
    assert.deepEqual(badge('good'), { id: 'check', color: 'charts.blue' });
    assert.deepEqual(badge('fair'), { id: 'warning', color: 'charts.yellow' });
    assert.deepEqual(badge('poor'), { id: 'error', color: 'charts.red' });
    assert.deepEqual(badge('nonsense'), badge('poor'), 'an unknown band is not silently green');
});

test('a pillar score falls in the same bands the server scores with', () => {
    assert.equal(gradeOf(100), 'excellent');
    assert.equal(gradeOf(80), 'excellent');
    assert.equal(gradeOf(79.9), 'good');
    assert.equal(gradeOf(50), 'good');
    assert.equal(gradeOf(49.9), 'fair');
    assert.equal(gradeOf(25), 'fair');
    assert.equal(gradeOf(24.9), 'poor');
});

test('a well-formed answer is accepted', () => {
    assert.ok(isFunctionHealth(health));
    assert.ok(isFunctionHealth({ uri: 'file:///a.rs', functions: [] }), 'a file with no functions');
});

test('an answer from a server this client does not understand is rejected', () => {
    assert.equal(isFunctionHealth(undefined), false);
    assert.equal(isFunctionHealth(null), false);
    assert.equal(isFunctionHealth({ functions: [] }), false, 'no uri');
    assert.equal(isFunctionHealth({ uri: 'file:///a.rs' }), false, 'no functions');
    assert.equal(
        isFunctionHealth({ uri: 'file:///a.rs', functions: [{ name: 'add' }] }),
        false,
        'a function without its numbers',
    );
    assert.equal(
        isFunctionHealth({
            uri: 'file:///a.rs',
            functions: [{ ...detail(), quality: 'excellent' }],
        }),
        false,
        'quality that is not a number',
    );
    assert.equal(
        isFunctionHealth({
            uri: 'file:///a.rs',
            functions: [{ ...detail(), pillars: [{ name: 'size', score: 12 }] }],
        }),
        false,
        'a pillar without its measures',
    );
    assert.equal(
        isFunctionHealth({
            uri: 'file:///a.rs',
            functions: [{ ...detail(), quality: Number.NaN }],
        }),
        false,
        'a score that would draw as NaN%',
    );
});
