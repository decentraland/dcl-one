import { join } from 'node:path';
import { sentryVitePlugin } from '@sentry/vite-plugin';
import react from '@vitejs/plugin-react';
import vendors from '../.electron-vendors.cache.json' with { type: 'json' };
import { rendererExpose } from '../preload/expose-plugin.js';

const PACKAGE_ROOT = import.meta.dirname;
const PACKAGE_DIR = join(PACKAGE_ROOT, '..');

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
      // @mui/icons-material 5 has no exports map, so a deep import resolves to its CommonJS file. Vite 8
      // gives a default import of that file the module object, not the component. The esm folder holds ES modules.
      '@mui/icons-material': '@mui/icons-material/esm',
      // decentraland-ui2 imports `@dcl/hooks` in a component that Creator Hub does not use. The workspace
      // removes that package, because it brings ua-parser-js 2 (AGPL) into the bundle.
      '@dcl/hooks': join(PACKAGE_ROOT, 'src', 'lib', 'dcl-hooks-stub.ts').replaceAll('\\', '/'),
    },
  },
  base: '',
  server: {
    fs: {
      strict: true,
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
