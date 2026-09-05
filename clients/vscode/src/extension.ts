import { existsSync } from 'node:fs';

import type { ExtensionContext, StatusBarItem } from 'vscode';
import {
    ExtensionMode,
    MarkdownString,
    StatusBarAlignment,
    ThemeColor,
    commands,
    window,
    workspace,
} from 'vscode';
import {
    LanguageClient,
    TransportKind,
    type LanguageClientOptions,
    type ServerOptions,
} from 'vscode-languageclient/node';

import { connectToServer, debugServerPort } from './debug.js';
import { describeMissingServer, resolveServerBinary } from './serverPath.js';
import { FILE_HEALTH_METHOD, isFileHealth, renderStatus, type FileHealth } from './status.js';

/** The settings section the server reads, and the client's own id. */
const SECTION = 'lspfAnalysis';

/**
 * What the server can analyze, as VS Code names it.
 *
 * These are the ids `language_for` in the server understands; anything else
 * would start the server for a document it cannot parse.
 */
const LANGUAGES = [
    'javascript',
    'javascriptreact',
    'python',
    'rust',
    'typescript',
    'typescriptreact',
];

let client: LanguageClient | undefined;

/** The latest file summary per document, keyed by URI. */
const reports = new Map<string, FileHealth>();
let statusItem: StatusBarItem | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
    statusItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
    statusItem.name = 'LSPF Analysis';
    statusItem.command = 'workbench.actions.view.problems';
    context.subscriptions.push(
        statusItem,
        commands.registerCommand(`${SECTION}.restartServer`, async () => {
            // Nothing to restart when the first start gave up on a missing
            // binary; try again instead, since the user may have just
            // installed one.
            if (client) {
                await client.restart();
            } else {
                await start(context);
            }
        }),
        window.onDidChangeActiveTextEditor(() => refreshStatus()),
        workspace.onDidCloseTextDocument((document) => {
            reports.delete(document.uri.toString());
            refreshStatus();
        }),
        workspace.onDidChangeConfiguration((event) => {
            if (event.affectsConfiguration(`${SECTION}.statusBar`)) {
                refreshStatus();
            }
        }),
    );
    await start(context);
}

/**
 * Shows the active file's health, or nothing when there is none to show.
 *
 * The status bar follows the editor rather than the last analysis: a summary
 * for a file in another tab would be worse than no summary at all.
 */
function refreshStatus(): void {
    if (!statusItem) {
        return;
    }
    const active = window.activeTextEditor?.document.uri.toString();
    const health = active === undefined ? undefined : reports.get(active);
    const enabled = workspace.getConfiguration(SECTION).get<boolean>('statusBar.enabled', true);
    if (!health || !enabled) {
        statusItem.hide();
        return;
    }

    const rendered = renderStatus(health);
    statusItem.text = rendered.text;
    statusItem.tooltip = new MarkdownString(rendered.tooltip);
    statusItem.backgroundColor = rendered.warning
        ? new ThemeColor('statusBarItem.warningBackground')
        : undefined;
    statusItem.show();
}

/**
 * How to reach the server: a socket to one already running under a
 * debugger, or a process of our own over stdio.
 *
 * Returns nothing when the binary is missing, having said so — starting
 * anyway would leave the client retrying a spawn that cannot succeed.
 */
function serverOptionsFor(context: ExtensionContext): ServerOptions | undefined {
    const port = debugServerPort(process.env);
    if (port !== undefined) {
        return () => connectToServer(port);
    }

    const resolved = resolveServerBinary({
        extensionPath: context.extensionPath,
        development: context.extensionMode === ExtensionMode.Development,
        configuredPath: workspace.getConfiguration(SECTION).get<string>('server.path'),
    });
    if (!existsSync(resolved.binary)) {
        void window.showErrorMessage(describeMissingServer(resolved));
        return undefined;
    }
    return {
        command: resolved.binary,
        args: ['serve', '--stdio'],
        transport: TransportKind.stdio,
        options: {
            env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? 'info' },
        },
    };
}

async function start(context: ExtensionContext): Promise<void> {
    const serverOptions = serverOptionsFor(context);
    if (!serverOptions) {
        return;
    }

    const clientOptions: LanguageClientOptions = {
        documentSelector: LANGUAGES.map((language) => ({ language, scheme: 'file' })),
        // The server only accepts pushed configuration: without this the
        // client would send `didChangeConfiguration` with a null payload and
        // every setting below would be ignored.
        synchronize: { configurationSection: SECTION },
        // Settings the server can use before the first document arrives,
        // ahead of the push that follows initialization.
        initializationOptions: { [SECTION]: workspace.getConfiguration().get(SECTION) },
        outputChannelName: 'LSPF Analysis',
    };

    client = new LanguageClient(SECTION, 'LSPF Analysis', serverOptions, clientOptions);
    context.subscriptions.push(client);
    // Registered before starting, so the summary for the first document
    // opened cannot arrive before anything is listening for it.
    context.subscriptions.push(
        client.onNotification(FILE_HEALTH_METHOD, (params: unknown) => {
            if (!isFileHealth(params)) {
                return;
            }
            reports.set(params.uri, params);
            refreshStatus();
        }),
    );
    await client.start();
}

export function deactivate(): Thenable<void> | undefined {
    reports.clear();
    return client?.stop();
}
