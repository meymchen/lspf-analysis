import { existsSync } from 'node:fs';

import type { ExtensionContext, StatusBarItem } from 'vscode';
import {
  ExtensionMode,
  Hover,
  MarkdownString,
  Position,
  Range,
  Selection,
  StatusBarAlignment,
  ThemeColor,
  Uri,
  ViewColumn,
  commands,
  env,
  l10n,
  window,
  workspace,
} from 'vscode';
import {
  DidChangeConfigurationNotification,
  LanguageClient,
  TransportKind,
  type LanguageClientOptions,
  type ServerOptions,
} from 'vscode-languageclient/node';

import {
  connectToServer,
  connectToWebSocketServer,
  debugServerPort,
  debugServerTransport,
} from './debug.js';
import { followServerLog } from './serverLog.js';
import {
  FUNCTION_HEALTH_METHOD,
  isFunctionHealth,
  type FunctionHealth,
  type Sort,
} from './functions.js';
import { t, useTranslator } from './i18n.js';
import { SERVER_ARGS, describeMissingServer, resolveServerBinary } from './serverPath.js';
import {
  FILE_HEALTH_METHOD,
  GO_TO_FUNCTION_COMMAND,
  isFileHealth,
  renderStatus,
  type FileHealth,
} from './status.js';
import { FunctionHealthProvider } from './tree.js';

/** The settings section the server reads, and the client's own id. */
const SECTION = 'lspfAnalysis';

/**
 * The commands the rendered Markdown is allowed to invoke.
 *
 * `isTrusted` takes an allowlist rather than a blanket `true` so a malicious
 * server cannot smuggle an arbitrary command link into a hover.
 */
const ENABLED_COMMANDS = [
  GO_TO_FUNCTION_COMMAND,
  `${SECTION}.restartServer`,
  'workbench.actions.view.problems',
  'workbench.action.openSettings',
];

/**
 * What the server can analyze, as VS Code names it.
 *
 * These are the ids `language_for` in the server understands; anything else
 * would start the server for a document it cannot parse.
 */
const LANGUAGES = [
  'java',
  'javascript',
  'javascriptreact',
  'python',
  'cpp',
  'rust',
  'typescript',
  'typescriptreact',
];

/** The view id the tree is contributed under, matching `package.json`. */
const FUNCTIONS_VIEW = `${SECTION}.functions`;

let client: LanguageClient | undefined;
let outputChannel: import('vscode').OutputChannel;

/** The latest file summary per document, keyed by URI. */
const reports = new Map<string, FileHealth>();
let statusItem: StatusBarItem | undefined;
/** What the status bar was last set to, so it is not set to it again. */
let shown: { text: string; tooltip: string; warning: boolean } | undefined;
let tree: FunctionHealthProvider | undefined;

/**
 * The active document's URI, when it is one the server can analyze.
 *
 * Asking about a document in an unsupported language would spend a round trip
 * to be told the server has nothing, on every editor change.
 */
function analyzableUri(): string | undefined {
  const document = window.activeTextEditor?.document;
  if (!document || !LANGUAGES.includes(document.languageId)) {
    return undefined;
  }
  return document.uri.toString();
}

/**
 * Asks the server for one document's per-function breakdown.
 *
 * A server that does not know the method answers with an error rather than a
 * payload — `lspfAnalysis.server.path` can point at any build, including one
 * older than this view — and so does one that has not finished starting. Both
 * mean the same thing here: nothing to draw yet.
 */
async function fetchFunctionHealth(uri: string): Promise<FunctionHealth | undefined> {
  if (!client) {
    return undefined;
  }
  try {
    const answer = await client.sendRequest<unknown>(FUNCTION_HEALTH_METHOD, { uri });
    return isFunctionHealth(answer) ? answer : undefined;
  } catch {
    return undefined;
  }
}

/**
 * The settings to send the server, with the editor's display language added.
 *
 * The language is not one of the extension's settings — it belongs to the
 * editor — but the server renders hovers and diagnostic messages, so it has
 * to be told. It rides in the same section so that both the startup options
 * and every later push carry it; a push without it would leave the server
 * rendering English from the next configuration change onward.
 */
function settingsPayload(): Record<string, unknown> {
  return {
    ...workspace.getConfiguration().get<Record<string, unknown>>(SECTION),
    locale: env.language,
  };
}

/** Puts the view in one of its two orders, and tells the title bar which. */
function setSort(sort: Sort): void {
  tree?.setSort(sort);
  void commands.executeCommand('setContext', `${SECTION}.sort`, sort);
}

export async function activate(context: ExtensionContext): Promise<void> {
  // Before anything renders: until this runs, every string is the English
  // it was written in.
  useTranslator(l10n.t);

  outputChannel = window.createOutputChannel('LSPF Analysis');
  context.subscriptions.push(outputChannel);
  const debugLog = process.env.LSPF_ANALYSIS_DEBUG_LOG;
  if (debugServerPort(process.env) !== undefined && debugLog) {
    context.subscriptions.push(followServerLog(debugLog, outputChannel));
  }

  tree = new FunctionHealthProvider(fetchFunctionHealth, analyzableUri);
  setSort('quality');

  statusItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
  statusItem.name = 'LSPF Analysis';
  // A status bar item with no command gets no rich hover at all
  // (microsoft/vscode#126753), so this is load-bearing, not decoration.
  statusItem.command = 'workbench.actions.view.problems';

  context.subscriptions.push(
    statusItem,
    tree,
    window.createTreeView(FUNCTIONS_VIEW, {
      treeDataProvider: tree,
      showCollapseAll: true,
    }),
    commands.registerCommand(`${SECTION}.sortByQuality`, () => setSort('quality')),
    commands.registerCommand(`${SECTION}.sortByPosition`, () => setSort('position')),
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
    commands.registerCommand(GO_TO_FUNCTION_COMMAND, goToFunction),
    window.onDidChangeActiveTextEditor(() => {
      refreshStatus();
      tree?.refresh();
    }),
    workspace.onDidCloseTextDocument((document) => {
      reports.delete(document.uri.toString());
      refreshStatus();
      tree?.refresh();
    }),
    workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration(`${SECTION}.statusBar`)) {
        refreshStatus();
      }
    }),
  );
  await start(context);
}

/** Opens `uri` with the cursor on a 1-based `line`. */
async function goToFunction(uri: string, line: number): Promise<void> {
  const document = await workspace.openTextDocument(Uri.parse(uri));
  const editor = await window.showTextDocument(document, {
    viewColumn: ViewColumn.Active,
    preserveFocus: false,
  });
  const position = new Position(Math.max(0, line - 1), 0);
  editor.selection = new Selection(position, position);
  editor.revealRange(new Range(position, position));
}

/** Builds a Markdown string the editor renders with icons, colour and links. */
function rich(markdown: string): MarkdownString {
  const rendered = new MarkdownString(markdown, true);
  rendered.isTrusted = { enabledCommands: ENABLED_COMMANDS };
  // The hover is rebuilt here rather than taken from the client, so the
  // client's own `supportHtml` never reaches it: without this the `<span>`
  // the server was told it could send would be stripped back out again.
  rendered.supportHtml = true;
  return rendered;
}

/**
 * Shows the active file's health, or nothing when there is none to show.
 *
 * The status bar follows the editor rather than the last analysis: a summary
 * for a file in another tab would be worse than no summary at all.
 *
 * Assigning `tooltip` while its hover is open closes the hover instead of
 * refreshing it (microsoft/vscode#128887), and the server re-publishes on
 * every keystroke, so nothing is assigned unless it actually changed. That
 * is what lets the pointer travel from the item onto the hover.
 */
function refreshStatus(): void {
  if (!statusItem) {
    return;
  }
  const active = window.activeTextEditor?.document.uri.toString();
  const health = active === undefined ? undefined : reports.get(active);
  const enabled = workspace.getConfiguration(SECTION).get<boolean>('statusBar.enabled', true);
  if (!health || !enabled) {
    shown = undefined;
    statusItem.hide();
    return;
  }

  const rendered = renderStatus(health);
  if (
    shown?.text === rendered.text &&
    shown.tooltip === rendered.tooltip &&
    shown.warning === rendered.warning
  ) {
    return;
  }
  shown = rendered;

  statusItem.text = rendered.text;
  statusItem.tooltip = rich(rendered.tooltip);
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
    const connect =
      debugServerTransport(process.env) === 'ws' ? connectToWebSocketServer : connectToServer;
    return () => connect(port);
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
    // The transport contributes `--stdio`; see SERVER_ARGS.
    args: [...SERVER_ARGS],
    transport: TransportKind.stdio,
    options: {
      env: {
        ...process.env,
        // Stdio stderr is captured by LanguageClient into our Output Channel.
        LSPF_ANALYSIS_LOG_FILE: undefined,
        RUST_LOG:
          process.env.RUST_LOG ??
          (context.extensionMode === ExtensionMode.Development ? 'debug' : 'info'),
      },
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
    initializationOptions: { [SECTION]: settingsPayload() },
    outputChannel,
    traceOutputChannel: outputChannel,
    // Advertises `general.markdown.allowedTags`, which is how the server
    // learns it may colour a grade letter with a `<span>`. A client that
    // does not say this gets the letter on its own, so the capability is
    // the whole of the agreement.
    markdown: { supportHtml: true },
    middleware: {
      workspace: {
        // The default push sends the configuration section verbatim,
        // which would drop the language the server renders in.
        didChangeConfiguration: async () => {
          await client?.sendNotification(DidChangeConfigurationNotification.type, {
            settings: { [SECTION]: settingsPayload() },
          });
        },
      },
      // The server writes Markdown every LSP client can render, which
      // rules out icons and command links. Re-wrap it here so VS Code
      // renders the icons, and append the actions only it can offer.
      provideHover: async (document, position, token, next) => {
        const hover = await next(document, position, token);
        if (!hover) {
          return hover;
        }
        const contents = hover.contents.map((part) => {
          // A fenced code block is left alone: it is meant to be
          // read as code, not decorated.
          if (typeof part !== 'string' && 'language' in part) {
            return part;
          }
          const markdown = typeof part === 'string' ? part : part.value;
          return rich(`${markdown}\n\n${hoverActions()}`);
        });
        return new Hover(contents, hover.range);
      },
    },
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
      // The summary says the file was re-analyzed; the breakdown behind
      // it is fetched only if the view is open to ask for it.
      tree?.onFileHealth(params);
    }),
  );
  await client.start();
}

/** The footer appended to every hover, in VS Code's own idiom. */
function hoverActions(): string {
  const settings = `command:workbench.action.openSettings?${encodeURIComponent(
    JSON.stringify(['lspfAnalysis.health']),
  )}`;
  return (
    `[$(list-flat) ${t('Problems')}](command:workbench.actions.view.problems)` +
    `  ·  [$(gear) ${t('Thresholds')}](${settings})`
  );
}

export function deactivate(): Thenable<void> | undefined {
  reports.clear();
  shown = undefined;
  tree = undefined;
  return client?.stop();
}
