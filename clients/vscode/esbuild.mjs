import { build, context } from 'esbuild';

const options = {
    entryPoints: ['src/extension.ts'],
    bundle: true,
    external: ['vscode'],
    format: 'cjs',
    logLevel: 'info',
    minify: false,
    outfile: 'dist/extension.js',
    platform: 'node',
    sourcemap: true,
    target: 'node20',
};

if (process.argv.includes('--watch')) {
    const buildContext = await context(options);
    await buildContext.watch();
} else {
    await build(options);
}
