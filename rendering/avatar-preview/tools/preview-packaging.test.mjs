import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';

const helper = await readFile(new URL('../../bevy-explorer/source/deploy/web/engine/content_cache.js', import.meta.url));

async function fixture(t, missingHelper = false) {
  const root = await mkdtemp(join(process.env.HARNESS_BUILD_DIR || tmpdir(), 'preview-packaging-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const script = join(root, 'preview', 'build.sh');
  await mkdir(join(root, 'preview'));
  await copyFile(new URL('../build.sh', import.meta.url), script);
  const source = join(root, 'repo', 'rendering/bevy-explorer/source/deploy/web/engine');
  await mkdir(source, { recursive: true });
  if (!missingHelper) await writeFile(join(source, 'content_cache.js'), helper);
  const wasmPack = join(root, 'fake-wasm-pack');
  await writeFile(wasmPack, `#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
  case "$1" in
    --out-dir) out=$2; shift 2 ;;
    *) shift ;;
  esac
done
mkdir -p "$out"
printf 'fake wasm' > "$out/preview.wasm"
`, { mode: 0o755 });
  const output = join(root, 'output root');
  return {
    output,
    build: (...args) => spawnSync('sh', [script, '--fast', ...args], {
      cwd: root,
      encoding: 'utf8',
      env: {
        ...process.env,
        DCL_ONE_ROOT: join(root, 'repo'),
        AVATAR_PREVIEW_OUT: output,
        WASM_PACK: wasmPack,
      },
    }),
  };
}

test('both backend packages contain the exact cache helper', async (t) => {
  const { build, output } = await fixture(t);
  const result = build();
  assert.equal(result.status, 0, result.stderr);
  for (const backend of ['pkg', 'pkg-gl']) {
    assert.deepEqual(await readFile(join(output, backend, 'content_cache.js')), helper);
    assert.equal(await readFile(join(output, backend, 'preview.wasm'), 'utf8'), 'fake wasm');
  }
});

test('webgpu-only packages its helper without creating a WebGL2 package', async (t) => {
  const { build, output } = await fixture(t);
  const result = build('--webgpu-only');
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(await readFile(join(output, 'pkg', 'content_cache.js')), helper);
  await assert.rejects(access(join(output, 'pkg-gl')), { code: 'ENOENT' });
});

test('a missing cache helper fails the package build', async (t) => {
  const { build } = await fixture(t, true);
  const result = build();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /content_cache\.js/);
});
