import * as os from 'node:os';
import * as path from 'node:path';

import { t } from './i18n.js';

/** Where the binary came from, which decides what to say when it is missing. */
export type ServerSource = 'configured' | 'development' | 'bundled';

export interface ResolvedServer {
  binary: string;
  source: ServerSource;
}

/** The server binary's name, which only Windows spells differently. */
export function executableName(platform: NodeJS.Platform): string {
  return platform === 'win32' ? 'lspf-analysis.exe' : 'lspf-analysis';
}

/**
 * What the server process is started with, short of the transport flag.
 *
 * `--stdio` is deliberately absent: `TransportKind.stdio` has
 * vscode-languageclient append it to `Executable.args` on the way to `spawn`,
 * and the server's parser refuses the flag twice — which failed the launch
 * with `the argument '--stdio' cannot be used multiple times` before the
 * client had said a word.
 */
export const SERVER_ARGS: readonly string[] = ['serve'];

/**
 * The command line that actually reaches a server spawned over stdio.
 *
 * Mirrors what vscode-languageclient adds, so the argv can be checked, and
 * run against the real binary, without an editor to host the extension.
 */
export function stdioArgs(args: readonly string[] = SERVER_ARGS): string[] {
  return [...args, '--stdio'];
}

export interface ServerLocation {
  extensionPath: string;
  /** True when running out of an Extension Development Host. */
  development: boolean;
  /** What `lspfAnalysis.server.path` is set to, if anything. */
  configuredPath?: string;
  platform?: NodeJS.Platform;
  homeDirectory?: string;
}

/**
 * Decides which binary to launch, most specific source first.
 *
 * A configured path always wins, since someone who set it means it. Failing
 * that, a development host runs the debug build from the workspace — the
 * extension lives two levels below the repository root — and an installed
 * extension runs the one packaged beside it.
 */
export function resolveServerBinary({
  extensionPath,
  development,
  configuredPath,
  platform = process.platform,
  homeDirectory = os.homedir(),
}: ServerLocation): ResolvedServer {
  const configured = configuredPath?.trim();
  if (configured) {
    return {
      binary: path.resolve(expandHome(configured, homeDirectory)),
      source: 'configured',
    };
  }
  const executable = executableName(platform);
  return development
    ? {
        binary: path.resolve(extensionPath, '..', '..', 'target', 'debug', executable),
        source: 'development',
      }
    : { binary: path.join(extensionPath, 'server', executable), source: 'bundled' };
}

/** Expands a leading `~`, which a hand-written setting is likely to contain. */
export function expandHome(candidate: string, homeDirectory: string): string {
  if (candidate === '~') {
    return homeDirectory;
  }
  if (candidate.startsWith('~/') || candidate.startsWith('~\\')) {
    return path.join(homeDirectory, candidate.slice(2));
  }
  return candidate;
}

/**
 * Explains a binary that is not there.
 *
 * For a packaged extension the likely cause is a VSIX built for another
 * architecture, which VS Code installs without complaint when it is handed
 * the file directly; saying so is more useful than reporting a bare path.
 */
export function describeMissingServer({ binary, source }: ResolvedServer): string {
  const found = t('LSPF Analysis could not find its language server at {0}.', binary);
  switch (source) {
    case 'configured':
      return `${found} ${t('Check the lspfAnalysis.server.path setting.')}`;
    case 'development':
      return `${found} ${t('Run cargo build in the repository first.')}`;
    case 'bundled':
      return `${found} ${t(
        'This usually means the installed extension was built for a different ' +
          'platform. Install the build matching this machine, or point ' +
          'lspfAnalysis.server.path at an lspf-analysis binary.',
      )}`;
  }
}
