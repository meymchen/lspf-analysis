/**
 * The Function Health view: the file's functions, worst first, expandable
 * down to the individual measure that set each pillar.
 *
 * A hover answers about the function under the pointer, one at a time, and
 * shares its popup with every other hover provider. This view answers about
 * the whole file at once and keeps its own space, which is what makes it
 * worth having alongside.
 */

import {
    EventEmitter,
    MarkdownString,
    ThemeColor,
    ThemeIcon,
    TreeItem,
    TreeItemCollapsibleState,
    type Event,
    type TreeDataProvider,
} from 'vscode';

import {
    badge,
    functionDescription,
    functionTooltip,
    gradeOf,
    measureDescription,
    measureTooltip,
    pillarDescription,
    sortFunctions,
    type FunctionDetail,
    type FunctionHealth,
    type Measure,
    type Pillar,
    type Sort,
} from './functions.js';
import { functionLabel, measureLabel, pillarLabel } from './i18n.js';
import { GO_TO_FUNCTION_COMMAND, type FileHealth } from './status.js';

/**
 * One row of the tree.
 *
 * Every row carries an `id`, because a refresh builds new objects and VS Code
 * has nothing else to recognize a row by. Without them the tree would
 * collapse itself every time the server republishes, which is every time the
 * reader stops typing.
 */
export type Node =
    | { kind: 'function'; id: string; uri: string; detail: FunctionDetail }
    | { kind: 'pillar'; id: string; pillar: Pillar }
    | { kind: 'measure'; id: string; measure: Measure };

/** How the view asks the server for a document's breakdown. */
export type Fetch = (uri: string) => Promise<FunctionHealth | undefined>;

/** What the view is looking at: the active document, or nothing. */
export type ActiveUri = () => string | undefined;

/**
 * How long to wait before asking again after a republish.
 *
 * The server re-analyzes on every keystroke and announces it with a file
 * summary; refetching the whole breakdown that often would be a request per
 * character typed. Long enough to cover typing, short enough that the view is
 * never visibly stale.
 */
const REFRESH_DELAY_MS = 300;

export class FunctionHealthProvider implements TreeDataProvider<Node> {
    private readonly changed = new EventEmitter<Node | undefined>();
    readonly onDidChangeTreeData: Event<Node | undefined> = this.changed.event;

    private sort: Sort = 'quality';
    /** The answer on screen, so expanding a row costs no round trip. */
    private loaded: FunctionHealth | undefined;
    private pending: ReturnType<typeof setTimeout> | undefined;
    /**
     * Which fetch is current. An answer that arrives after the reader moved
     * to another file is dropped rather than drawn under its name.
     */
    private generation = 0;

    constructor(
        private readonly fetch: Fetch,
        private readonly activeUri: ActiveUri,
    ) {}

    /** Redraws now: the document or the configuration changed. */
    refresh(): void {
        this.cancelPending();
        this.invalidate();
    }

    /**
     * Redraws shortly, coalescing the republishes that arrive while typing.
     *
     * What is on screen is left standing until the new answer is asked for,
     * so the rows do not blink empty between keystrokes.
     */
    scheduleRefresh(): void {
        if (this.pending) {
            return;
        }
        this.pending = setTimeout(() => {
            this.pending = undefined;
            this.invalidate();
        }, REFRESH_DELAY_MS);
    }

    /** Reorders what is on screen without asking the server again. */
    setSort(sort: Sort): void {
        if (this.sort === sort) {
            return;
        }
        this.sort = sort;
        this.changed.fire(undefined);
    }

    /** Notices a republish, ignoring the ones for other documents. */
    onFileHealth(health: FileHealth): void {
        if (health.uri === this.activeUri()) {
            this.scheduleRefresh();
        }
    }

    dispose(): void {
        this.cancelPending();
        this.changed.dispose();
    }

    private invalidate(): void {
        this.loaded = undefined;
        this.generation += 1;
        this.changed.fire(undefined);
    }

    private cancelPending(): void {
        if (this.pending) {
            clearTimeout(this.pending);
            this.pending = undefined;
        }
    }

    async getChildren(node?: Node): Promise<Node[]> {
        if (!node) {
            return this.roots();
        }
        if (node.kind === 'function') {
            return node.detail.pillars.map((pillar) => ({
                kind: 'pillar' as const,
                id: `${node.id}/${pillar.name}`,
                pillar,
            }));
        }
        if (node.kind === 'pillar') {
            return node.pillar.measures.map((measure) => ({
                kind: 'measure' as const,
                id: `${node.id}/${measure.name}`,
                measure,
            }));
        }
        return [];
    }

    getTreeItem(node: Node): TreeItem {
        switch (node.kind) {
            case 'function':
                return functionItem(node);
            case 'pillar':
                return pillarItem(node);
            case 'measure':
                return measureItem(node);
        }
    }

    /** Fetches the active document's functions, at most once per refresh. */
    private async roots(): Promise<Node[]> {
        const uri = this.activeUri();
        if (uri === undefined) {
            return [];
        }
        if (this.loaded?.uri !== uri) {
            const generation = this.generation;
            const health = await this.fetch(uri);
            if (generation !== this.generation || health?.uri !== uri) {
                return [];
            }
            this.loaded = health;
        }

        // Keyed by position in the file rather than by name: two anonymous
        // functions can share a name and a line, and an index cannot collide.
        const ids = new Map<FunctionDetail, string>(
            this.loaded.functions.map((detail, index) => [detail, `function:${index}`]),
        );
        return sortFunctions(this.loaded.functions, this.sort).map((detail) => ({
            kind: 'function' as const,
            id: ids.get(detail) ?? `function:${detail.startLine}`,
            uri,
            detail,
        }));
    }
}

/** Builds the icon for a band, tinted by the theme's chart colours. */
function icon(grade: string): ThemeIcon {
    const { id, color } = badge(grade);
    return new ThemeIcon(id, new ThemeColor(color));
}

function markdown(value: string): MarkdownString {
    // Theme icons, but not trusted: nothing here carries a command link, and
    // a tooltip has no need to be able to run one.
    return new MarkdownString(value, true);
}

function functionItem(node: Node & { kind: 'function' }): TreeItem {
    const item = new TreeItem(functionLabel(node.detail.name), TreeItemCollapsibleState.Collapsed);
    item.id = node.id;
    item.description = functionDescription(node.detail);
    item.iconPath = icon(node.detail.grade);
    item.tooltip = markdown(functionTooltip(node.detail));
    item.contextValue = 'lspfAnalysis.function';
    item.command = {
        command: GO_TO_FUNCTION_COMMAND,
        title: 'Go to Function',
        arguments: [node.uri, node.detail.startLine],
    };
    return item;
}

function pillarItem(node: Node & { kind: 'pillar' }): TreeItem {
    const item = new TreeItem(
        pillarLabel(node.pillar.name),
        node.pillar.measures.length > 0
            ? TreeItemCollapsibleState.Collapsed
            : TreeItemCollapsibleState.None,
    );
    item.id = node.id;
    item.description = pillarDescription(node.pillar);
    item.iconPath = icon(gradeOf(node.pillar.score));
    item.contextValue = 'lspfAnalysis.pillar';
    return item;
}

function measureItem(node: Node & { kind: 'measure' }): TreeItem {
    const item = new TreeItem(measureLabel(node.measure.name), TreeItemCollapsibleState.None);
    item.id = node.id;
    item.description = measureDescription(node.measure);
    item.iconPath = icon(gradeOf(node.measure.score));
    item.tooltip = markdown(measureTooltip(node.measure));
    item.contextValue = 'lspfAnalysis.measure';
    return item;
}
