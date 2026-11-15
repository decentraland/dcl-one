import { join } from 'node:path';
import { sentryVitePlugin } from '@sentry/vite-plugin';
import react from '@vitejs/plugin-react';
import { searchForWorkspaceRoot } from 'vite';
import vendors from '../.electron-vendors.cache.json' with { type: 'json' };
import { rendererExpose } from '../preload/expose-plugin.js';

const PACKAGE_ROOT = import.meta.dirname;
const PACKAGE_DIR = join(PACKAGE_ROOT, '..');
const UI3_SRC = join(PACKAGE_ROOT, '../../../../ui3/src').replaceAll('\\', '/');
// The ui3 LAND map imports the shared Shop model through ui3's `@shop` alias.
const SHOP_SRC = join(PACKAGE_ROOT, '../../../../../library/shop').replaceAll('\\', '/');

// The renderer tests run in UTC, so a date that code makes in local time does not depend on the
// zone of the machine. This file runs before Vitest starts a worker, which is where Vitest says to
// set the zone: a worker thread does not apply a `TZ` that a setup file or `test.env` gives it.
if (process.env.VITEST) process.env.TZ = 'UTC';

/**
 * @type {import('vite').UserConfig}
 * @see https://vitejs.dev/config/
 */
const config = {
  mode: process.env.MODE,
  root: PACKAGE_ROOT,
  envDir: PACKAGE_DIR,
  resolve: {
    alias: {
      '/@/': join(PACKAGE_ROOT, 'src').replaceAll('\\', '/') + '/',
      '/shared/': join(PACKAGE_ROOT, '../shared').replaceAll('\\', '/') + '/',
      '/assets/': join(PACKAGE_ROOT, 'assets').replaceAll('\\', '/') + '/',
      '#store': join(PACKAGE_ROOT, 'src', 'modules', 'store').replaceAll('\\', '/') + '/',
      '@ui': UI3_SRC,
      '@shop': SHOP_SRC,
    },
    // The ui3 sources sit outside this workspace and must use the React installed here.
    dedupe: ['react', 'react-dom'],
  },
  base: '',
  server: {
    fs: {
      strict: true,
      allow: [searchForWorkspaceRoot(PACKAGE_ROOT), UI3_SRC, SHOP_SRC],
    },
  },
  build: {
    sourcemap: true,
    target: `chrome${vendors.chrome}`,
    outDir: 'dist',
    assetsDir: '.',
    rolldownOptions: {
      input: [join(PACKAGE_ROOT, 'index.html')],
    },
    emptyOutDir: true,
    reportCompressedSize: false,
  },
  test: {
    environment: 'happy-dom',
    // CSS modules are injected into the test document, so a spec can read their computed styles.
    css: { include: /\.module\.css$/ },
    setupFiles: ['./tests/setupTests.ts'],
    include: ['./src/**/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}'],
  },
  plugins: [
    react(),
    rendererExpose({
      preloadEntry: join(PACKAGE_ROOT, '../preload/src/index.ts'),
    }),
    sentryVitePlugin({
      org: 'decentraland',
      project: 'creator-hub',
      disable: process.env.MODE === 'development' || process.env.DRY_RUN,
    }),
  ],
};

export default config;
