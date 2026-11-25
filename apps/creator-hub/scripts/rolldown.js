#!/usr/bin/env node
// Starts Vite, Vitest or oxlint from a package script with the environment that rolldown-env.js sets:
// `node ../../scripts/rolldown.js vitest run`.
import './rolldown-env.js';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const name = process.argv[2];
const require = createRequire(pathToFileURL(resolve('package.json')));
const manifest = require.resolve(`${name}/package.json`);
const { bin } = require(manifest);
const entry = resolve(dirname(manifest), typeof bin === 'string' ? bin : bin[name]);
// The command reads its arguments from process.argv, as if it had been started directly.
process.argv.splice(1, 2, entry);
await import(pathToFileURL(entry).href);
