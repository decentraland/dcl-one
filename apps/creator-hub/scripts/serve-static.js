#!/usr/bin/env node
// Serves a folder at http://localhost:<port> with the static handler of the Creator Hub app:
// `node ../../scripts/serve-static.js public 8000`. The inspector e2e workflow starts it.
// The handler is a TypeScript file that Node loads directly, so this needs Node 22.18 or later.
import { createServer } from 'node:http';
import { createStaticHandler } from '../packages/creator-hub/main/src/modules/static-server.ts';

const [root, port] = process.argv.slice(2);
if (!root || !/^\d+$/.test(port ?? '')) {
  console.error('Usage: serve-static.js <folder> <port>');
  process.exit(1);
}

const handler = createStaticHandler({
  root,
  headers: {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Headers': 'Origin, X-Requested-With, Content-Type, Accept, Range',
  },
});

createServer(handler).listen(Number(port), 'localhost', () => {
  console.log(`Serving ${root} at http://localhost:${port}`);
});
