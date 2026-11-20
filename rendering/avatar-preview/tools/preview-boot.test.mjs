// Run with node --experimental-vm-modules --test tools/preview-boot.test.mjs.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';

const previewSource = await readFile(new URL('../web/preview.js', import.meta.url), 'utf8');
const cacheSource = await readFile(new URL('../../bevy-explorer/source/deploy/web/engine/content_cache.js', import.meta.url), 'utf8');

async function preview(backend, missingCache = false) {
  const messages = [];
  const imports = [];
  const trims = [];
  let started = false;
  const window = {
    innerWidth: 700, innerHeight: 700,
    addEventListener() {},
    postMessage(message) { messages.push(message); },
  };
  window.parent = window;
  const location = new URL(`https://preview.invalid/?backend=${backend}&disableDefaultEmotes=true`);
  const context = createContext({
    window, location, navigator: {}, URL, URLSearchParams, setTimeout, performance,
    document: {
      body: { classList: { toggle() {} } },
      documentElement: { style: { setProperty() {} } },
      getElementById() { return null; },
    },
    console: { log() {}, warn() {}, error() {} },
    fetch: async () => ({ ok: true, json: async () => ({ build: 'fake-build' }) }),
  });
  const module = new SourceTextModule(previewSource, {
    context,
    initializeImportMeta(meta) { meta.url = 'https://preview.invalid/preview.js'; },
    async importModuleDynamically(url) {
      imports.push(url);
      let dependency;
      if (url.includes('/content_cache.js?')) {
        if (missingCache) throw new Error('cache helper unavailable');
        dependency = new SourceTextModule(cacheSource, { context });
      } else {
        assert.match(url, /dcl_avatar_preview\.js\?v=fake-build$/);
        dependency = new SyntheticModule(['default', 'start', 'preview_command'], function () {
          this.setExport('default', async () => {});
          this.setExport('start', () => {
            assert.equal(typeof window.__contentCacheTrim, 'function');
            assert.equal(typeof window.__contentCacheTouch, 'function');
            trims.push(window.__contentCacheTrim(1024, false, []));
            started = true;
          });
          this.setExport('preview_command', () => {});
        }, { context });
      }
      await dependency.link(() => assert.fail('unexpected static import'));
      await dependency.evaluate();
      return dependency;
    },
  });
  await module.link(() => assert.fail('unexpected static import'));
  await module.evaluate();
  return { boot: () => module.namespace.boot({ canvas: '#preview' }), messages, imports, trims, started: () => started };
}

for (const backend of ['webgpu', 'webgl2']) {
  test(`${backend} installs the cache page functions before starting without a service worker`, async () => {
    const page = await preview(backend);
    await page.boot();
    assert.equal(page.started(), true);
    assert.equal(await page.trims[0], null);
    const directory = backend === 'webgpu' ? 'pkg' : 'pkg-gl';
    assert.equal(page.imports[0], `https://preview.invalid/${directory}/content_cache.js?v=fake-build`);
  });
}

test('a missing cache helper reports a host error before starting the engine', async () => {
  const page = await preview('webgpu', true);
  await assert.rejects(page.boot(), /cache helper unavailable/);
  assert.equal(page.started(), false);
  assert.ok(page.messages.some(message => message.type === 'error'));
});
