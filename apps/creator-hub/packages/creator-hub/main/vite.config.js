import { join } from 'node:path';
import { sentryVitePlugin } from '@sentry/vite-plugin';
import vendors from '../.electron-vendors.cache.json' with { type: 'json' };

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
      '/@/': join(PACKAGE_ROOT, 'src') + '/',
      '/shared/': join(PACKAGE_ROOT, '../shared') + '/',
    },
  },
  build: {
    ssr: true,
    sourcemap: 'inline',
    target: `node${vendors.node}`,
    outDir: 'dist',
    assetsDir: '.',
    minify: process.env.MODE !== 'development',
    lib: {
      entry: 'src/index.ts',
      formats: ['es'],
    },
    rolldownOptions: {
      output: {
        entryFileNames: '[name].js',
        // Keep one output file. Without this, rolldown writes a runtime chunk next to the entry.
        codeSplitting: false,
      },
    },
    emptyOutDir: true,
    reportCompressedSize: false,
  },
  plugins: [
    sentryVitePlugin({
      org: 'decentraland',
      project: 'creator-hub',
      disable: process.env.MODE === 'development' || process.env.DRY_RUN,
    }),
  ],
};

export default config;
