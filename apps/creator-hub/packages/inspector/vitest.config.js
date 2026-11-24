import { resolve } from 'path';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  assetsInclude: ['**/*.glb', '**/*.gltf'],
  // Specs parse with oxc directly, never through the browser wasm fallback.
  define: { INSPECTOR_DEV_PARSER: 'false' },
  test: {
    globals: true,
    environment: 'happy-dom',
    // happy-dom 21 gives an iframe a window only when page loading is on. Specs read `contentWindow`.
    environmentOptions: { happyDOM: { settings: { enableIframePageLoading: true } } },
    setupFiles: ['./test/setup.ts'],
    include: ['src/**/*.spec.ts', 'src/**/*.spec.tsx'],
    exclude: ['e2e/**/*.spec.ts'],
    testTimeout: 10000,
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
      '@ui': resolve(__dirname, '../../../ui3/src'),
      '@dcl/one-ui': resolve(__dirname, '../../../../library/ui'),
    },
    // The ui3 sources sit outside this workspace and must use the React installed here.
    dedupe: ['react', 'react-dom'],
  },
  optimizeDeps: {
    exclude: [
      '@babylonjs/core',
      '@babylonjs/gui',
      '@babylonjs/inspector',
      '@babylonjs/loaders',
      '@babylonjs/materials',
    ],
  },
  ssr: {
    noExternal: ['@dcl/gltf-validator-ts'],
  },
});
