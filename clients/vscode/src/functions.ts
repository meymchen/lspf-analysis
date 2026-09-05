/**
 * The per-function breakdown, and how the tree labels it.
 *
 * Nothing here imports `vscode`: this is the shape the server sends and the
 * strings drawn from it, which is what the tests exercise. Turning those into
 * tree items is `tree.ts`.
 */

import { functionLabel, gradeLabel, measureLabel, pillarLabel, t } from './i18n.js';
import { bar, icon } from './status.js';

/** The `lspfAnalysis/functionHealth` request the server answers per file. */
export const FUNCTION_HEALTH_METHOD = 'lspfAnalysis/functionHealth';

export interface Measure {
    name: string;
    /** The raw value the engine computed. */
    value: number;
    /** The value that would score 50%. */
    threshold: number;
    score: number;
}

export interface Pillar {
    name: string;
    /** The worst of `measures`, never their average. */
    score: number;
    measures: Measure[];
}

export interface FunctionDetail {
    name: string;
    /** Where it starts and ends, 1-based. */
    startLine: number;
    endLine: number;
    quality: number;
    grade: string;
    weakestPillar: string;
    weakestMetric: string;
    pillars: Pillar[];
}

export interface FunctionHealth {
    uri: string;
    /** Every function of the file, in source order. */
    functions: FunctionDetail[];
}

/** How the tree orders the file's functions. */
export type Sort = 'quality' | 'position';

/**
 * Orders the functions for display, without disturbing the server's answer.
 *
 * Worst first by default, because the extension's whole claim is that it
 * tells you which functions are getting hard to work with; source order is
 * there for reading alongside the file.
 */
export function sortFunctions(functions: readonly FunctionDetail[], sort: Sort): FunctionDetail[] {
    const ordered = [...functions];
    if (sort === 'quality') {
        // Ties break toward the earlier function, so the order is stable
        // between keystrokes rather than shuffling on every republish.
        ordered.sort((a, b) => a.quality - b.quality || a.startLine - b.startLine);
    } else {
        ordered.sort((a, b) => a.startLine - b.startLine);
    }
    return ordered;
}

/** A codicon and the theme colour to tint it with. */
export interface Badge {
    id: string;
    color: string;
}

/**
 * The icon standing in for a band in the tree.
 *
 * The same four codicons the status bar uses, tinted with the chart colours
 * rather than the error ones: a fair function is worth looking at, not a
 * problem the editor is reporting. `charts.*` is a documented theme colour,
 * so it follows whatever theme the reader has on.
 */
export function badge(grade: string): Badge {
    switch (grade) {
        case 'excellent':
            return { id: 'pass', color: 'charts.green' };
        case 'good':
            return { id: 'check', color: 'charts.blue' };
        case 'fair':
            return { id: 'warning', color: 'charts.yellow' };
        default:
            return { id: 'error', color: 'charts.red' };
    }
}

/** Rounds a metric the way the hover does, so the two never disagree. */
const rounded = (value: number) => Math.round(value);

/** The band a score falls in, by the same cuts the server scores with. */
export function gradeOf(score: number): string {
    if (score >= 80) {
        return 'excellent';
    }
    if (score >= 50) {
        return 'good';
    }
    if (score >= 25) {
        return 'fair';
    }
    return 'poor';
}

/** The line beside a function's name: what it scored, and what dragged it. */
export function functionDescription(detail: FunctionDetail): string {
    return `${rounded(detail.quality)}%  ·  ${pillarLabel(detail.weakestPillar)}`;
}

/** The line beside a pillar's name. */
export function pillarDescription(pillar: Pillar): string {
    return `${rounded(pillar.score)}%`;
}

/**
 * The line beside a measure's name.
 *
 * The raw value against its threshold, then what that scored — a reader can
 * see both what was counted and how it was judged.
 */
export function measureDescription(measure: Measure): string {
    return `${rounded(measure.value)} / ${rounded(measure.threshold)}  ·  ${rounded(measure.score)}%`;
}

/** The Markdown shown when the pointer rests on a function. */
export function functionTooltip(detail: FunctionDetail): string {
    const lines = [
        `${icon(detail.grade)} **\`${functionLabel(detail.name)}\`**  ·  ` +
            `${t('quality')} **${rounded(detail.quality)}%**  ·  ${gradeLabel(detail.grade)}`,
        '',
        bar(detail.quality),
        '',
        t('Lines {0}–{1}.', detail.startLine, detail.endLine),
    ];
    if (detail.weakestMetric) {
        lines.push(
            '',
            t(
                'Weakest: **{0}** — {1}.',
                pillarLabel(detail.weakestPillar),
                measureLabel(detail.weakestMetric),
            ),
        );
    }
    return lines.join('\n');
}

/** The Markdown shown when the pointer rests on a measure. */
export function measureTooltip(measure: Measure): string {
    return [
        `**${measureLabel(measure.name)}**  ·  ${rounded(measure.score)}%`,
        '',
        bar(measure.score),
        '',
        t(
            'Measured {0} against a threshold of {1}, the value that would score 50%.',
            rounded(measure.value),
            rounded(measure.threshold),
        ),
    ].join('\n');
}

const isMeasure = (value: unknown): value is Measure => {
    const candidate = value as Record<string, unknown> | null;
    return (
        typeof candidate === 'object' &&
        candidate !== null &&
        typeof candidate.name === 'string' &&
        ['value', 'threshold', 'score'].every(
            (key) => typeof candidate[key] === 'number' && Number.isFinite(candidate[key]),
        )
    );
};

const isPillar = (value: unknown): value is Pillar => {
    const candidate = value as Record<string, unknown> | null;
    return (
        typeof candidate === 'object' &&
        candidate !== null &&
        typeof candidate.name === 'string' &&
        typeof candidate.score === 'number' &&
        Array.isArray(candidate.measures) &&
        candidate.measures.every(isMeasure)
    );
};

const isFunctionDetail = (value: unknown): value is FunctionDetail => {
    const candidate = value as Record<string, unknown> | null;
    return (
        typeof candidate === 'object' &&
        candidate !== null &&
        typeof candidate.name === 'string' &&
        typeof candidate.grade === 'string' &&
        typeof candidate.weakestPillar === 'string' &&
        typeof candidate.weakestMetric === 'string' &&
        ['startLine', 'endLine', 'quality'].every(
            (key) => typeof candidate[key] === 'number' && Number.isFinite(candidate[key]),
        ) &&
        Array.isArray(candidate.pillars) &&
        candidate.pillars.every(isPillar)
    );
};

/**
 * Checks that a response is the shape this client expects.
 *
 * The server is versioned separately from the extension — a user can point
 * `lspfAnalysis.server.path` at any build — so an unrecognizable answer draws
 * an empty tree rather than a tree of `NaN%`.
 */
export function isFunctionHealth(value: unknown): value is FunctionHealth {
    if (typeof value !== 'object' || value === null) {
        return false;
    }
    const candidate = value as Record<string, unknown>;
    return (
        typeof candidate.uri === 'string' &&
        Array.isArray(candidate.functions) &&
        candidate.functions.every(isFunctionDetail)
    );
}
