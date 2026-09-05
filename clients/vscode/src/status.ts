/** The `lspfAnalysis/fileHealth` notification the server pushes per file. */
export const FILE_HEALTH_METHOD = 'lspfAnalysis/fileHealth';

export interface WorstFunction {
    name: string;
    quality: number;
    grade: string;
    /** Where it starts, 1-based. */
    line: number;
}

export interface FileHealth {
    uri: string;
    quality: number;
    grade: string;
    functions: number;
    /** How many functions already carry a diagnostic. */
    below: number;
    worst?: WorstFunction | null;
}

export interface StatusText {
    text: string;
    tooltip: string;
    /** Set only when the file is bad enough to be worth colouring. */
    warning: boolean;
}

/** The codicon standing in for each band. */
function icon(grade: string): string {
    switch (grade) {
        case 'excellent':
            return '$(pass)';
        case 'good':
            return '$(check)';
        case 'fair':
            return '$(warning)';
        default:
            return '$(error)';
    }
}

const plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;

/**
 * Renders a file's health for the status bar.
 *
 * The bar itself only has room for the band and the number; everything a
 * reader would act on goes in the tooltip.
 */
export function renderStatus(health: FileHealth): StatusText {
    const quality = Math.round(health.quality);
    const lines = [
        `**LSPF Analysis** — file quality **${quality}%** (${health.grade})`,
        '',
        `${plural(health.functions, 'function')} analyzed, ${health.below} below the warning threshold.`,
    ];
    if (health.worst) {
        lines.push(
            '',
            `Worst: \`${health.worst.name}\` at ${Math.round(health.worst.quality)}% ` +
                `(${health.worst.grade}), line ${health.worst.line}.`,
        );
    }
    return {
        text: `${icon(health.grade)} ${quality}%`,
        tooltip: lines.join('\n'),
        warning: health.grade === 'poor' || health.grade === 'fair',
    };
}

/**
 * Checks that a notification payload is the shape this client expects.
 *
 * The server is versioned separately from the extension — a user can point
 * `lspfAnalysis.server.path` at any build — so an unrecognizable payload is
 * ignored rather than rendered as `NaN%`.
 */
export function isFileHealth(value: unknown): value is FileHealth {
    if (typeof value !== 'object' || value === null) {
        return false;
    }
    const candidate = value as Record<string, unknown>;
    return (
        typeof candidate.uri === 'string' &&
        typeof candidate.quality === 'number' &&
        Number.isFinite(candidate.quality) &&
        typeof candidate.grade === 'string' &&
        typeof candidate.functions === 'number' &&
        typeof candidate.below === 'number'
    );
}
