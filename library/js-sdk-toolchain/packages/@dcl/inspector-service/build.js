#!/usr/bin/env node
const child_process = require('child_process');
const path = require('path');
const { future } = require('fp-future');
const esbuild = require('esbuild');
const { getNotBundledModules } = require('./build-externals.js');

const WATCH_MODE = process.argv.includes('--watch');
const PRODUCTION = process.argv.includes('--production');

async function main() {
  const context = await esbuild.context({
    entryPoints: ['src/index.ts'],
    bundle: true,
    platform: 'node',
    outfile: 'dist/index.js',
    sourcemap: PRODUCTION ? false : 'both',
    minify: PRODUCTION,
    external: getNotBundledModules(),
  });

  if (WATCH_MODE) {
    await context.watch();
  } else {
    console.time('> Building NodeJs bundle');
    await context.rebuild();
    await context.dispose();
    console.timeEnd('> Building NodeJs bundle');
  }

  await runTypeChecker();
}

function runTypeChecker() {
  // TypeScript 7 exports no lib/tsc; its package.json locates the launcher.
  const tsc = path.join(path.dirname(require.resolve('typescript/package.json')), 'bin', 'tsc');
  const args = [tsc, '-p', 'tsconfig.json'];
  if (WATCH_MODE) args.push('--watch');

  console.time('> Running typechecker');
  const ts = child_process.spawn('node', args, {
    env: process.env,
    cwd: process.cwd(),
    encoding: 'utf8',
  });
  const typeCheckerFuture = future();

  ts.on('close', code => {
    console.timeEnd('> Running typechecker');
    console.log('  Type checker exit code:', code);
    if (code !== 0) {
      typeCheckerFuture.reject(new Error(`Typechecker exited with code ${code}.`));
      return;
    }

    typeCheckerFuture.resolve(code);
  });

  ts.stdout.pipe(process.stdout);
  ts.stderr.pipe(process.stderr);

  if (WATCH_MODE) {
    typeCheckerFuture.resolve();
  }

  return typeCheckerFuture;
}

main().catch(err => {
  process.exitCode = 1;
  console.error(err);
  process.exit(1);
});
