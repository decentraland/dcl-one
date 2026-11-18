// Runs index.ts through a Vite dev server's module runner, with the main process's aliases and
// the Electron stubs from vite.config.js:
//   node scripts/assistant-manifest/run.mjs <out.json> <app version> <dcl-one commit>
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, createServerModuleRunner } from 'vite';

const here = dirname(fileURLToPath(import.meta.url));
const server = await createServer({
  configFile: join(here, 'vite.config.js'),
  appType: 'custom',
  server: { middlewareMode: true, hmr: false, ws: false },
  logLevel: 'warn',
});
try {
  await createServerModuleRunner(server.environments.ssr, { hmr: false }).import(
    join(here, 'index.ts'),
  );
} finally {
  await server.close();
}
