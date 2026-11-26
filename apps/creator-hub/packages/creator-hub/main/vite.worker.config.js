import { join } from 'node:path';
import vendors from '../.electron-vendors.cache.json' with { type: 'json' };

const PACKAGE_ROOT = import.meta.dirname;
const PACKAGE_DIR = join(PACKAGE_ROOT, '..');

// Second main-process bundle: the optimizer worker (src/optimizer-worker.ts). It runs on the
// bundled real Node, not in Electron, so it gets its own entry instead of a chunk of index.js.
// The toolchain is external on purpose — it is downloaded on first use into userData
// (modules/optimizer/tools.ts) and resolved from there at runtime, never inlined here.

/**
 * @type {import('vite').UserConfig}
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
      entry: 'src/optimizer-worker.ts',
      formats: ['es'],
    },
    rolldownOptions: {
      external: ['sharp', 'meshoptimizer', /^@gltf-transform\//],
      output: {
        entryFileNames: '[name].js',
        // Keep one output file. The app copies this bundle alone into the tools folder.
        codeSplitting: false,
      },
    },
    emptyOutDir: false,
    reportCompressedSize: false,
  },
};

export default config;
