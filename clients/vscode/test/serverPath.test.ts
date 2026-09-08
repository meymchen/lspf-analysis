import * as assert from 'node:assert/strict';
import * as path from 'node:path';
import test from 'node:test';

import {
    SERVER_ARGS,
    describeMissingServer,
    executableName,
    expandHome,
    resolveServerBinary,
    stdioArgs,
} from '../src/serverPath.js';

test('adds the executable suffix only on Windows', () => {
    assert.equal(executableName('linux'), 'lspf-analysis');
    assert.equal(executableName('darwin'), 'lspf-analysis');
    assert.equal(executableName('win32'), 'lspf-analysis.exe');
});

test('resolves the debug server two levels above the extension', () => {
    assert.deepEqual(
        resolveServerBinary({
            extensionPath: '/repo/clients/vscode',
            development: true,
            platform: 'linux',
        }),
        { binary: path.resolve('/repo/target/debug/lspf-analysis'), source: 'development' },
    );
});

test('resolves the server bundled in a production VSIX', () => {
    assert.deepEqual(
        resolveServerBinary({
            extensionPath: '/ext/lspf-analysis',
            development: false,
            platform: 'win32',
        }),
        { binary: path.join('/ext/lspf-analysis/server/lspf-analysis.exe'), source: 'bundled' },
    );
});

test('a configured POSIX path wins over both other sources', { skip: process.platform === 'win32' }, () => {
    for (const development of [true, false]) {
        assert.deepEqual(
            resolveServerBinary({
                extensionPath: '/repo/clients/vscode',
                development,
                configuredPath: '/usr/local/bin/lspf-analysis',
                platform: 'linux',
            }),
            { binary: '/usr/local/bin/lspf-analysis', source: 'configured' },
        );
    }
});

test('a configured Windows path wins over both other sources', { skip: process.platform !== 'win32' }, () => {
    for (const development of [true, false]) {
        assert.deepEqual(
            resolveServerBinary({
                extensionPath: 'C:\\repo\\clients\\vscode',
                development,
                configuredPath: 'C:\\tools\\lspf-analysis.exe',
                platform: 'win32',
            }),
            { binary: 'C:\\tools\\lspf-analysis.exe', source: 'configured' },
        );
    }
});

test('a blank configured path is treated as unset', () => {
    assert.equal(
        resolveServerBinary({
            extensionPath: '/repo/clients/vscode',
            development: false,
            configuredPath: '   ',
            platform: 'linux',
        }).source,
        'bundled',
    );
});

test('a configured path expands a leading tilde', () => {
    assert.equal(
        resolveServerBinary({
            extensionPath: '/repo/clients/vscode',
            development: false,
            configuredPath: '~/.cargo/bin/lspf-analysis',
            platform: 'linux',
            homeDirectory: '/home/dev',
        }).binary,
        path.resolve('/home/dev/.cargo/bin/lspf-analysis'),
    );
});

test('a tilde anywhere but the front is left alone', () => {
    assert.equal(expandHome('/opt/~backup/lspf-analysis', '/home/dev'), '/opt/~backup/lspf-analysis');
    assert.equal(expandHome('~', '/home/dev'), '/home/dev');
});

test('the missing-server message names the path and the likely cause', () => {
    const bundled = describeMissingServer({ binary: '/ext/server/lspf-analysis', source: 'bundled' });
    assert.match(bundled, /\/ext\/server\/lspf-analysis/);
    assert.match(bundled, /different platform/);

    assert.match(
        describeMissingServer({ binary: '/nope', source: 'configured' }),
        /lspfAnalysis\.server\.path/,
    );
    assert.match(
        describeMissingServer({ binary: '/nope', source: 'development' }),
        /cargo build/,
    );
});

test('the transport flag is contributed once, by the transport', () => {
    // `TransportKind.stdio` appends `--stdio` to `Executable.args` itself.
    // Passing it as well made the server refuse to start at all, with
    // "the argument '--stdio' cannot be used multiple times", which reads
    // as a broken connection rather than as a bad command line.
    assert.ok(!SERVER_ARGS.includes('--stdio'), SERVER_ARGS.join(' '));
    assert.deepEqual(stdioArgs(), ['serve', '--stdio']);
    assert.equal(stdioArgs().filter((argument) => argument === '--stdio').length, 1);
});
