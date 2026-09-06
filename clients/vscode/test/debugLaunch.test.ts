import * as assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as path from 'node:path';
import test from 'node:test';

const root = path.resolve(__dirname, '../../../..');
const launch = JSON.parse(readFileSync(path.join(root, '.vscode/launch.json'), 'utf8'));
const tasks = JSON.parse(readFileSync(path.join(root, '.vscode/tasks.json'), 'utf8')).tasks;

test('the debug menu exposes exactly stdio, tcp, and websocket without prompts', () => {
    const visible = [...launch.configurations, ...launch.compounds]
        .filter(config => !config.presentation?.hidden)
        .sort((a, b) => a.presentation.order - b.presentation.order);
    assert.deepEqual(visible.map(config => config.name), ['stdio', 'tcp', 'websocket']);
    assert.equal(launch.inputs, undefined);
    for (const config of launch.configurations) {
        assert.equal(config.serverReadyAction, undefined);
        assert.equal(config.initCommands, undefined);
        assert.equal(config.exitCommands, undefined);
    }
    for (const config of visible) {
        const prepare = tasks.find((task: { label: string }) => task.label === config.preLaunchTask);
        assert.equal(prepare.dependsOrder, 'sequence');
        assert.deepEqual(prepare.dependsOn, ['build language server', 'prepare extension development host']);
    }
});

test('stdio host clears inherited network debug settings', () => {
    const host = launch.configurations.find((config: { name: string }) => config.name === 'stdio');
    assert.equal(host.type, 'extensionHost');
    assert.ok(host.args.includes('--extensionDevelopmentPath=${workspaceFolder}/clients/vscode'));
    for (const variable of ['LSPF_ANALYSIS_DEBUG_PORT', 'LSPF_ANALYSIS_DEBUG_TCP', 'LSPF_ANALYSIS_DEBUG_TRANSPORT', 'LSPF_ANALYSIS_DEBUG_LOG']) {
        assert.equal(host.env[variable], null);
    }
});

for (const [name, protocol, port] of [['tcp', 'tcp', '9257'], ['websocket', 'ws', '9258']]) {
    test(`${name} launches matching server and host processes after preparation`, () => {
        const compound = launch.compounds.find((config: { name: string }) => config.name === name);
        assert.equal(compound.configurations.length, 2);
        assert.equal(compound.stopAll, true);
        const parts = compound.configurations.map((name: string) =>
            launch.configurations.find((config: { name: string }) => config.name === name));
        const server = parts.find((config: { type: string }) => config.type === 'lldb');
        const host = parts.find((config: { type: string }) => config.type === 'extensionHost');
        assert.ok(server);
        assert.ok(host);
        assert.deepEqual(server.args, ['serve', `--${protocol}`, `127.0.0.1:${port}`]);
        assert.equal(server.terminal, 'integrated');
        assert.equal(host.env.LSPF_ANALYSIS_DEBUG_PORT, port);
        assert.equal(host.env.LSPF_ANALYSIS_DEBUG_TRANSPORT, protocol);
        for (const part of parts) {
            assert.equal(part.presentation.hidden, true);
            assert.equal(part.preLaunchTask, undefined, 'build both parts before starting either process');
        }
    });
}
