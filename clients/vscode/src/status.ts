import { functionLabel, gradeLabel, measureLabel, pillarLabel, t } from './i18n.js';

/** The `lspfAnalysis/fileHealth` notification the server pushes per file. */
export const FILE_HEALTH_METHOD = 'lspfAnalysis/fileHealth';

/** Opens a source location. Registered by the extension. */
export const GO_TO_SOURCE_COMMAND = 'lspfAnalysis.goToSource';

export interface WorstFunction {
  name: string;
  quality: number;
  grade: string;
  /** Where it starts, 1-based. */
  line: number;
  weakestPillar: string;
  weakestMetric: string;
}

export interface Bands {
  excellent: number;
  good: number;
  fair: number;
  poor: number;
}

export interface FileHealth {
  uri: string;
  quality: number;
  grade: string;
  functions: number;
  /** How many functions already carry a diagnostic. */
  below: number;
  bands: Bands;
  /** Worst first, capped by the server's `worstFunctions` setting. */
  worst: WorstFunction[];
}

export interface StatusText {
  text: string;
  tooltip: string;
  /** Set only when the file is bad enough to be worth colouring. */
  warning: boolean;
}

/** How many cells a score bar is drawn with. */
const BAR_CELLS = 10;

/** The partial cells, from one eighth of a cell to seven eighths. */
const EIGHTHS = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/**
 * Draws a 0-100 score as a bar, for the status bar's tooltip.
 *
 * The partial cells come from the same Unicode block as the full one and are
 * designed to tile, so ten cells resolve eighty steps without the bar growing
 * any wider.
 *
 * Never wrapped in a code span: VS Code draws inline code with a background
 * and horizontal padding, which would put a gap in the middle of the bar
 * wherever the span began.
 */
export function bar(score: number): string {
  const cells = BAR_CELLS * 8;
  // A NaN score draws an empty bar rather than a row of `undefined`.
  const eighths = Math.min(cells, Math.max(0, Math.round((score / 100) * cells) || 0));
  const full = Math.floor(eighths / 8);
  const partial = eighths % 8;
  return (
    '█'.repeat(full) +
    (partial > 0 ? EIGHTHS[partial - 1] : '') +
    '░'.repeat(BAR_CELLS - full - (partial > 0 ? 1 : 0))
  );
}

/** The codicon standing in for each band. */
export function icon(grade: string): string {
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

/** Encodes command arguments the way a `command:` URI needs them. */
function commandLink(command: string, args: unknown[]): string {
  return `command:${command}?${encodeURIComponent(JSON.stringify(args))}`;
}

/**
 * Renders the band spread as one line of bars.
 *
 * A single percentage says how the file scores; this says how it is shaped,
 * which is what tells a reader whether one bad function is dragging an
 * otherwise healthy file down.
 */
function bandLine(bands: Bands, total: number): string {
  if (total === 0) {
    return '';
  }
  const cells = 20;
  const order: Array<[keyof Bands, string]> = [
    ['excellent', '█'],
    ['good', '▓'],
    ['fair', '▒'],
    ['poor', '░'],
  ];
  let drawn = '';
  for (const [band, glyph] of order) {
    drawn += glyph.repeat(Math.round((bands[band] / total) * cells));
  }
  return drawn.slice(0, cells).padEnd(cells, '░');
}

/**
 * Renders a file's health for the status bar.
 *
 * The bar itself has room for the band and the number; everything a reader
 * would act on goes in the tooltip, which VS Code renders as Markdown and
 * lets the pointer move onto.
 */
export function renderStatus(health: FileHealth): StatusText {
  const quality = Math.round(health.quality);
  const attention = health.below > 0 ? `  $(alert)${health.below}` : '';

  const lines = [
    `**LSPF Analysis**  ·  ${icon(health.grade)} **${quality}%**  ·  ${gradeLabel(health.grade)}`,
    '',
    bar(health.quality),
    '',
  ];

  if (health.functions === 0) {
    lines.push(t('No functions to analyze in this file.'));
  } else {
    lines.push(
      // Singular and plural are separate source strings rather than a
      // suffix, because a language without plurals cannot be given one
      // by appending to a translation.
      health.functions === 1
        ? t('{0} function, {1} below the warning threshold.', 1, health.below)
        : t('{0} functions, {1} below the warning threshold.', health.functions, health.below),
      '',
      bandLine(health.bands, health.functions),
      '',
      `| | ${t('band')} | ${t('functions')} |`,
      '| :-- | :-- | --: |',
      `| $(pass) | ${gradeLabel('excellent')} | ${health.bands.excellent} |`,
      `| $(check) | ${gradeLabel('good')} | ${health.bands.good} |`,
      `| $(warning) | ${gradeLabel('fair')} | ${health.bands.fair} |`,
      `| $(error) | ${gradeLabel('poor')} | ${health.bands.poor} |`,
    );
  }

  if (health.worst.length > 0) {
    lines.push('', '---', '', `**${t('Worth opening first')}**`, '');
    for (const worst of health.worst) {
      const link = commandLink(GO_TO_SOURCE_COMMAND, [health.uri, worst.line]);
      const title = t('Go to line {0}', worst.line);
      // The score and the verdict are one source string, so that the
      // punctuation between them belongs to the language it is read in
      // rather than to this template.
      const verdict = worst.weakestMetric
        ? t(
            '**{0}%**, weakest {1} ({2})',
            Math.round(worst.quality),
            pillarLabel(worst.weakestPillar),
            measureLabel(worst.weakestMetric),
          )
        : t('**{0}%**, weakest {1}', Math.round(worst.quality), pillarLabel(worst.weakestPillar));
      lines.push(
        `- ${icon(worst.grade)} [\`${functionLabel(worst.name)}\`](${link} "${title}")` +
          ` — ${verdict}`,
      );
    }
  }

  lines.push(
    '',
    '---',
    '',
    `[$(list-flat) ${t('Problems')}](command:workbench.actions.view.problems)` +
      `  ·  [$(gear) ${t('Settings')}](${commandLink('workbench.action.openSettings', [
        'lspfAnalysis',
      ])})` +
      `  ·  [$(refresh) ${t('Restart server')}](command:lspfAnalysis.restartServer)`,
  );

  return {
    text: `${icon(health.grade)} ${quality}%${attention}`,
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
  const bands = candidate.bands as Record<string, unknown> | undefined;
  return (
    typeof candidate.uri === 'string' &&
    typeof candidate.quality === 'number' &&
    Number.isFinite(candidate.quality) &&
    typeof candidate.grade === 'string' &&
    typeof candidate.functions === 'number' &&
    typeof candidate.below === 'number' &&
    Array.isArray(candidate.worst) &&
    typeof bands === 'object' &&
    bands !== null &&
    ['excellent', 'good', 'fair', 'poor'].every((band) => typeof bands[band] === 'number')
  );
}
