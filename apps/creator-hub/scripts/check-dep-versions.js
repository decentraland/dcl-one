#!/usr/bin/env node
// Fails when two workspace manifests declare different versions of one dependency:
// `node scripts/check-dep-versions.js`. Peer ranges are not compared.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const manifests = [
  'package.json',
  ...readdirSync(join(root, 'packages')).map(dir => `packages/${dir}/package.json`),
].filter(manifest => existsSync(join(root, manifest)));
const workspacePackages = new Set(['@dcl/asset-packs', '@dcl/inspector']);
const versions = new Map();

for (const manifest of manifests) {
  const pkg = JSON.parse(readFileSync(join(root, manifest), 'utf8'));
  for (const field of ['dependencies', 'devDependencies']) {
    for (const [name, version] of Object.entries(pkg[field] ?? {})) {
      if (workspacePackages.has(name)) continue;
      const seen = versions.get(name) ?? new Map();
      seen.set(version, [...(seen.get(version) ?? []), manifest]);
      versions.set(name, seen);
    }
  }
}

const mismatches = [...versions].filter(([, seen]) => seen.size > 1);
for (const [name, seen] of mismatches) {
  console.error(name);
  for (const [version, where] of seen) console.error(`  ${version}  ${where.join(', ')}`);
}
process.exit(mismatches.length > 0 ? 1 : 0);
