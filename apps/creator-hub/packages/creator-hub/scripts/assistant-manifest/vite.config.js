import { join } from 'node:path';

const ROOT = join(__dirname, '..', '..');
const STUB = join(__dirname, 'runtime-stub.ts');
// Imported by the tool definitions' module graph, and needing a running Electron app.
const STUBBED =
  /(^electron$|^electron-log\/main$|\/(window|mainWindow|explorer-gateway|realm-status)$)/;

// The main process's aliases, with the Electron-bound modules swapped for a stub: the script
// runs in plain Node.
export default {
  root: ROOT,
  resolve: {
    alias: {
      '/@/': join(ROOT, 'main', 'src') + '/',
      '/shared/': join(ROOT, 'shared') + '/',
    },
  },
  plugins: [
    {
      name: 'assistant-manifest-stubs',
      enforce: 'pre',
      resolveId: source => (STUBBED.test(source) ? STUB : null),
    },
  ],
};
