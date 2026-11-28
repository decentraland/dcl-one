#!/usr/bin/env node
/**
 * Before | After screenshots of the built Creator Hub app: one Electron launch per side, every
 * state of a states file walked in that session, then one Before | After grid per component.
 *
 * @example node ch-capture.mjs <states.mjs> --out <dir> --base <ref> [--only a,b] [--side before|after]
 * @example node ch-capture.mjs --self-test
 * Usage, the states file format and the capture rules: ../references/screenshots.md → "ch-capture".
 */
/* eslint-env node, browser */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { homedir, tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

const APP = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..');
const CH = 'packages/creator-hub';
const DISTS = ['main/dist', 'preload/dist', 'renderer/dist'];
const SOURCES = ['main', 'preload', 'renderer', 'shared'];
const READY = '[data-testid="logo-menu-button"]';
const VIEWPORT = { width: 1440, height: 900 };
const NO_UPDATE = { success: true, value: { updateAvailable: false, version: '0.0.0' } };
const PIN = [/^https:\/\/img\.youtube\.com\//];

function pinRoutes(page, patterns, dir) {
  mkdirSync(dir, { recursive: true });
  return Promise.all(
    patterns.map(pattern =>
      page.route(pattern, async route => {
        const file = join(dir, createHash('sha1').update(route.request().url()).digest('hex'));
        if (!existsSync(file)) {
          const response = await route.fetch();
          if (!response.ok()) return route.fulfill({ response });
          writeFileSync(`${file}.type`, response.headers()['content-type'] ?? '');
          writeFileSync(file, await response.body());
        }
        await route.fulfill({
          body: readFileSync(file),
          contentType: readFileSync(`${file}.type`, 'utf8'),
        });
      }),
    ),
  );
}

const { values: opts, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    out: { type: 'string' },
    base: { type: 'string' },
    'base-dir': { type: 'string' },
    only: { type: 'string' },
    side: { type: 'string', default: 'both' },
    'self-test': { type: 'boolean' },
  },
});

const log = (...args) => console.log('[ch-capture]', ...args);
const run = (cmd, args, cwd, env = {}) =>
  execFileSync(cmd, args, { cwd, stdio: 'inherit', env: { ...process.env, ...env } });
const git = (...args) => execFileSync('git', ['-C', APP, ...args], { encoding: 'utf8' }).trim();

async function timed(label, fn) {
  const start = Date.now();
  const result = await fn();
  log(`${label}: ${((Date.now() - start) / 1000).toFixed(1)} s`);
  return result;
}

const files = dir =>
  existsSync(dir)
    ? readdirSync(dir, { recursive: true, withFileTypes: true })
        .filter(entry => entry.isFile())
        .map(entry => join(entry.parentPath, entry.name))
    : [];

const newest = paths => paths.reduce((max, path) => Math.max(max, statSync(path).mtimeMs), 0);

/** Every VITE_SEGMENT_* name and non-empty value this machine can see: env and `.env*` files. */
function segmentEnv(app) {
  const names = new Set();
  const values = new Set();
  const add = (name, value) => {
    names.add(name);
    if (value) values.add(value);
  };
  for (const [name, value] of Object.entries(process.env)) {
    if (name.startsWith('VITE_SEGMENT_')) add(name, value);
  }
  const pkg = join(app, CH);
  for (const file of readdirSync(pkg).filter(name => name.startsWith('.env'))) {
    for (const line of readFileSync(join(pkg, file), 'utf8').split('\n')) {
      const match = line.match(/^\s*(?:export\s+)?(VITE_SEGMENT_\w+)\s*=\s*['"]?([^'"\s#]*)/);
      if (match) add(match[1], match[2]);
    }
  }
  return { names, values };
}

/** Bundle files under `app` that contain any of `values`. */
const leaks = (app, values) =>
  DISTS.flatMap(dist => files(join(app, CH, dist)))
    .filter(file => [...values].some(value => readFileSync(file, 'latin1').includes(value)))
    .map(file => relative(app, file));

const isStale = app => {
  const pkg = join(app, CH);
  const built = Math.min(...DISTS.map(dist => newest(files(join(pkg, dist)))));
  const source = newest(
    SOURCES.flatMap(dir => files(join(pkg, dir))).filter(
      file => !/[/\\](dist|node_modules|tests)[/\\]|\.(spec|test)\.tsx?$/.test(relative(pkg, file)),
    ),
  );
  return built < source;
};

function ensureBuilt(app, segment) {
  const leaked = leaks(app, segment.values);
  const stale = isStale(app);
  if (stale || leaked.length) {
    const env = {
      DRY_RUN: '1',
      VITE_SENTRY_DSN: '',
      ...Object.fromEntries([...segment.names].map(name => [name, ''])),
    };
    log(`building ${app} (${stale ? 'sources newer than dist' : 'dist carries a Segment key'})`);
    if (existsSync(join(app, 'packages/inspector/public/bundle.js'))) {
      run('npm', ['run', 'build'], join(app, CH), env);
    } else {
      run('make', ['protoc', 'build'], app, env);
    }
  }
  const after = leaks(app, segment.values);
  if (after.length) throw new Error(`Segment key found in the built bundle: ${after.join(', ')}`);
}

function baseCheckout(ref) {
  const sha = git('rev-parse', '--verify', `${ref}^{commit}`);
  const top = git('rev-parse', '--show-toplevel');
  const root = resolve(
    opts['base-dir'] ?? join(homedir(), '.cache', 'ch-capture', sha.slice(0, 12)),
  );
  const app = join(root, relative(top, APP));
  if (!existsSync(root)) {
    run('git', ['-C', top, 'worktree', 'add', '--detach', root, sha], top, {
      GIT_LFS_SKIP_SMUDGE: '1',
    });
  }
  if (!existsSync(join(app, 'node_modules/.package-lock.json'))) {
    run('git', ['lfs', 'pull', '--include', `${relative(top, APP)}/packages/asset-packs/**`], root);
    run('npm', ['ci'], app);
  }
  const vendors = join(CH, '.electron-vendors.cache.json');
  if (!existsSync(join(app, vendors))) copyFileSync(join(APP, vendors), join(app, vendors));
  return app;
}

const pngSize = file => {
  const header = readFileSync(file).subarray(16, 24);
  return { width: header.readUInt32BE(0), height: header.readUInt32BE(4) };
};

const stubSources = stubs =>
  Object.entries(stubs).map(([channel, stub]) => [
    channel,
    typeof stub === 'function' ? stub.toString() : `() => (${JSON.stringify(stub)})`,
  ]);

function swapStubs(electronApp, stubs) {
  return electronApp.evaluate(({ ipcMain }, list) => {
    const handlers = ipcMain._invokeHandlers;
    if (!(handlers instanceof Map)) throw new Error('ipcMain._invokeHandlers is gone');
    const originals = (globalThis.__chCaptureOriginals ??= new Map());
    for (const [channel, original] of originals) {
      ipcMain.removeHandler(channel);
      if (original) ipcMain.handle(channel, original);
    }
    originals.clear();
    for (const [channel, source] of list) {
      originals.set(channel, handlers.get(channel));
      ipcMain.removeHandler(channel);
      ipcMain.handle(channel, (0, eval)(`(${source})`));
    }
  }, stubSources(stubs));
}

function installProbe() {
  const find = (name, test) => {
    const host = [...document.querySelectorAll('body *')].find(element =>
      Object.keys(element).some(key => key.startsWith('__reactContainer$')),
    );
    const queue = host
      ? [host[Object.keys(host).find(key => key.startsWith('__reactContainer$'))]]
      : [];
    while (queue.length) {
      const fiber = queue.shift();
      if (fiber?.memoizedProps && test(fiber.memoizedProps)) return fiber.memoizedProps[name];
      if (fiber?.child) queue.push(fiber.child);
      if (fiber?.sibling) queue.push(fiber.sibling);
    }
    throw new Error(`no ${name} in the React tree`);
  };
  window.__chStore = () => find('store', props => props.store?.dispatch);
  window.__chNavigator = () => find('navigator', props => props.navigator?.push);
}

async function quietStart(page) {
  await page.locator(READY).waitFor({ timeout: 60_000 });
  await page.waitForFunction(() => {
    const { settings, featureFlags } = window.__chStore().getState();
    return [settings.checkForUpdates.status, featureFlags.status].every(
      status => !['idle', 'loading'].includes(status),
    );
  });
  await page.evaluate(() => {
    const store = window.__chStore();
    for (const { id } of store.getState().snackbar.notifications) {
      store.dispatch({ type: 'snackbar/removeSnackbar', payload: id });
    }
  });
}

function settle(page) {
  return page.evaluate(async () => {
    await document.fonts.ready;
    const urls = new Set([...document.images].map(image => image.currentSrc));
    for (const element of document.querySelectorAll('*')) {
      const css = getComputedStyle(element).backgroundImage;
      for (const [, url] of css.matchAll(/url\("?(.*?)"?\)/g)) urls.add(url);
      if (element instanceof HTMLVideoElement && element.poster) urls.add(element.poster);
    }
    await Promise.all(
      [...urls].filter(Boolean).map(url => {
        const image = new Image();
        image.src = url;
        return image.decode().catch(() => {});
      }),
    );
    if (!document.getElementById('ch-capture-caret')) {
      const style = document.createElement('style');
      style.id = 'ch-capture-caret';
      style.textContent = '* { caret-color: transparent !important; }';
      document.head.append(style);
    }
    for (const animation of document.getAnimations()) {
      if (animation.effect?.getComputedTiming().endTime === Infinity) animation.cancel();
      else animation.finish();
    }
    await new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done)));
  });
}

async function setLocale(page, locale) {
  await page.evaluate(locale => {
    window.__chStore().dispatch({ type: 'translation/changeLocale', payload: locale });
    window.__chNavigator().push('/ch-capture-remount');
  }, locale);
  await page.locator(READY).waitFor({ state: 'detached' });
  await page.evaluate(() => window.__chNavigator().go(-1));
  await page.locator(READY).waitFor();
}

const shotPath = (out, entry, side) => join(out, `${entry.prefix}-${side}.png`);

async function capture({ app, side, config, entries, out }) {
  const pkg = join(app, CH);
  const { _electron } = createRequire(join(APP, CH, 'package.json'))('playwright');
  const home = mkdtempSync(join(tmpdir(), 'ch-capture-home-'));
  const data = mkdtempSync(join(tmpdir(), 'ch-capture-data-'));
  mkdirSync(join(data, 'Scenes'));
  const failures = [];
  const electronApp = await timed(`${side} launch`, () =>
    _electron.launch({
      executablePath: createRequire(join(pkg, 'package.json'))('electron'),
      args: [
        pkg,
        `--user-data-dir=${data}`,
        '--force-color-profile=srgb',
        '--force-device-scale-factor=1',
      ],
      cwd: pkg,
      env: {
        ...process.env,
        HOME: home,
        SHELL: '/usr/bin/true',
        PATH: [config.bin, dirname(process.execPath), '/usr/bin', '/bin', '/usr/sbin', '/sbin']
          .filter(Boolean)
          .join(':'),
      },
      timeout: 60_000,
    }),
  );
  try {
    const page = await electronApp.firstWindow();
    await page.addInitScript(installProbe);
    await pinRoutes(page, config.pin ?? PIN, join(out, '.pinned'));
    await electronApp.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().forEach(window =>
        window.webContents.setImageAnimationPolicy('noAnimation'),
      ),
    );
    if (config.now) await page.clock.setFixedTime(new Date(config.now));
    await config.setup?.({ page, app: electronApp });
    const reset = async (ipc, viewport) => {
      await swapStubs(electronApp, { 'updater.checkForUpdates': NO_UPDATE, ...config.ipc, ...ipc });
      await page.setViewportSize(viewport ?? config.viewport ?? VIEWPORT);
      await page.reload();
      await quietStart(page);
    };
    await reset();
    await settle(page);
    for (const entry of entries) {
      const { state } = entry;
      const started = Date.now();
      try {
        await reset(state.ipc, state.viewport);
        if (state.locale && state.locale !== 'en') await setLocale(page, state.locale);
        await page.mouse.move(0, 0);
        await state.steps?.({ page, app: electronApp });
        await page.locator(state.ready).first().waitFor();
        await settle(page);
        await page.screenshot({
          path: shotPath(out, entry, side),
          scale: 'css',
          animations: 'disabled',
        });
        log(`${side} ${entry.prefix} ${Date.now() - started} ms`);
      } catch (error) {
        failures.push(entry.prefix);
        log(`${side} ${entry.prefix} FAILED: ${error.message.split('\n')[0]}`);
        await page
          .screenshot({ path: join(out, `_failed-${entry.prefix}-${side}.png`) })
          .catch(() => {});
      }
    }
  } finally {
    electronApp.process().kill('SIGKILL');
    await electronApp.close().catch(() => {});
    rmSync(home, { recursive: true, force: true });
    rmSync(data, { recursive: true, force: true });
  }
  return failures;
}

const escape = text => String(text).replace(/[&<>"]/g, char => `&#${char.charCodeAt(0)};`);

async function grids(out, entries) {
  const { chromium } = createRequire(join(APP, CH, 'package.json'))('playwright');
  const browser = await chromium.launch();
  const page = await browser.newPage({ deviceScaleFactor: 1 });
  const components = Map.groupBy(entries, entry => entry.state.component ?? entry.state.name);
  try {
    for (const [component, rows] of components) {
      const cell = file =>
        existsSync(file)
          ? `<img src="data:image/png;base64,${readFileSync(file).toString('base64')}">`
          : '<div class="missing">not captured</div>';
      const widths = rows.flatMap(entry =>
        ['before', 'after']
          .map(side => shotPath(out, entry, side))
          .filter(existsSync)
          .map(file => pngSize(file).width),
      );
      const column = Math.max(320, ...widths);
      const label = ({ prefix, state }) =>
        [prefix, state.locale ?? 'en', Object.values(state.viewport ?? VIEWPORT).join('×')].join(
          ' · ',
        );
      await page.setViewportSize({ width: column * 2 + 48, height: 600 });
      await page.setContent(`<!doctype html><style>
        body { margin: 0; padding: 16px; background: #1e1e1e; color: #ddd; font: 16px system-ui, sans-serif; }
        .grid { display: grid; grid-template-columns: ${column}px ${column}px; gap: 8px 16px; }
        h2 { grid-column: 1 / -1; margin: 16px 0 0; font: 600 18px ui-monospace, monospace; }
        h1 { margin: 0; font-size: 22px; }
        img { display: block; }
        .missing { padding: 48px; border: 1px dashed #666; color: #999; }
      </style><div class="grid"><h1>Before</h1><h1>After</h1>${rows
        .map(
          entry =>
            `<h2>${escape(label(entry))}</h2>${cell(shotPath(out, entry, 'before'))}${cell(shotPath(out, entry, 'after'))}`,
        )
        .join('')}</div>`);
      await page.evaluate(() => Promise.all([...document.images].map(image => image.decode())));
      await page.screenshot({ path: join(out, `grid-${component}.png`), fullPage: true });
      log(`grid-${component}.png: ${rows.length} rows`);
    }
  } finally {
    browser.close().catch(() => {});
  }
}

function selfTest() {
  const app = mkdtempSync(join(tmpdir(), 'ch-capture-self-test-'));
  try {
    const pkg = join(app, CH);
    for (const dist of DISTS) mkdirSync(join(pkg, dist), { recursive: true });
    writeFileSync(join(pkg, '.env.local'), 'VITE_SEGMENT_CREATORS_HUB_API_KEY=sk-example123\n');
    writeFileSync(join(pkg, 'main/dist/index.js'), 'const writeKey = "sk-example123";');
    const segment = segmentEnv(app);
    if (!segment.names.has('VITE_SEGMENT_CREATORS_HUB_API_KEY'))
      throw new Error('missed the key name');
    if (leaks(app, segment.values).join() !== `${CH}/main/dist/index.js`)
      throw new Error('missed a leak');
    writeFileSync(join(pkg, 'main/dist/index.js'), 'const writeKey = "";');
    if (leaks(app, segment.values).length) throw new Error('flagged a clean bundle');
    console.log('self-test ok');
  } finally {
    rmSync(app, { recursive: true, force: true });
  }
}

async function main() {
  if (opts['self-test']) return selfTest();
  const [statesFile] = positionals;
  if (!statesFile || !opts.out) {
    throw new Error(
      'usage: ch-capture.mjs <states.mjs> --out <dir> --base <ref> [--only a,b] [--side before|after]',
    );
  }
  const config = (await import(pathToFileURL(resolve(statesFile)).href)).default;
  const names = config.states.map(state => state.name);
  const duplicate = names.find((name, index) => names.indexOf(name) !== index);
  if (duplicate) throw new Error(`duplicate state name: ${duplicate}`);
  const all = config.states.map((state, index) => ({
    state,
    prefix: `${String(index + 1).padStart(2, '0')}-${state.name}`,
  }));
  const only = opts.only?.split(',');
  const unknown = only?.filter(name => !names.includes(name)) ?? [];
  if (unknown.length) throw new Error(`unknown state: ${unknown.join(', ')}`);
  const entries = only ? all.filter(entry => only.includes(entry.state.name)) : all;
  const sides = opts.side === 'both' ? ['before', 'after'] : [opts.side];
  if (sides.some(side => !['before', 'after'].includes(side)))
    throw new Error('--side is before, after or both');
  if (sides.includes('before') && !opts.base)
    throw new Error('--base <ref> is required for the before side');

  const out = resolve(opts.out);
  mkdirSync(out, { recursive: true });
  const segment = segmentEnv(APP);
  const failures = [];
  const total = Date.now();
  for (const side of sides) {
    const app =
      side === 'before' ? await timed('before checkout', () => baseCheckout(opts.base)) : APP;
    await timed(`${side} build`, () => ensureBuilt(app, segment));
    const failed = await timed(`${side} capture`, () =>
      capture({ app, side, config, entries, out }),
    );
    failures.push(...failed.map(name => `${name}-${side}`));
  }
  await timed('grids', () => grids(out, all));
  log(`round: ${((Date.now() - total) / 1000).toFixed(1)} s → ${out}`);
  if (failures.length) throw new Error(`failed: ${failures.join(', ')}`);
}

main().then(
  () => process.exit(0),
  error => {
    console.error('[ch-capture]', error.message);
    process.exit(1);
  },
);
