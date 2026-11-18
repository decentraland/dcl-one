import { join } from 'node:path';

const ROOT = join(import.meta.dirname, '..', '..');
const STUB = join(import.meta.dirname, 'runtime-stub.ts');
// Imported by the tool definitions' module graph, and needing a running Electron app.
const STUBBED =
  /(^electron$|^electron-log\/main$|\/(window|mainWindow|explorer-gateway|realm-status)$)/;

// The main process's aliases, with the Electron-bound modules swapped for a stub: the script
// runs in plain Node.
export default {
  root: ROOT,
  // Nothing here runs in a browser: no dependency scan or pre-bundling of the renderer's packages.
  optimizeDeps: { noDiscovery: true, entries: [], include: [] },
  resolve: {
    alias: {
      '/@/': join(ROOT, 'main', 'src') + '/',
      '/shared/': join(ROOT, 'shared') + '/',
    },
  },
  // Bare imports are left to Node unless bundled: these two must reach the stub above.
  ssr: { noExternal: ['electron', 'electron-log'] },
  plugins: [
    {
      name: 'assistant-manifest-stubs',
      enforce: 'pre',
      resolveId: source => (STUBBED.test(source) ? STUB : null),
    },
  ],
};
