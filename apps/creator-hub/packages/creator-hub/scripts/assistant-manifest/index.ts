// Writes `assistant-manifest.json` for a Creator Hub release: the system prompt, the tools and
// the skills denylist this build gives the AI assistant, with the same hashes the app puts in
// every shared turn (Assistant Insights matches them). Run through Vite (run.mjs), so it reads
// the sources with the main process's aliases:
//   node scripts/assistant-manifest/run.mjs <out.json> <app version> <dcl-one commit>
import fs from 'fs';

import { buildAssistantManifest } from '../../main/src/modules/ai-sharing/manifest';

const [out, appVersion, dclOneCommit] = process.argv.slice(2).filter(arg => arg !== '--');
if (!out || !appVersion || !/^[0-9a-f]{40}$/.test(dclOneCommit ?? '')) {
  console.error('usage: assistant-manifest.ts <out.json> <app version> <dcl-one commit sha>');
  process.exit(2);
}

const manifest = await buildAssistantManifest({ appVersion, dclOneCommit });
fs.writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(
  `assistant-manifest: ${manifest.tools.length} tools, prompt ${manifest.systemPromptHash.slice(0, 12)}, tools ${manifest.toolsHash.slice(0, 12)}`,
);
process.exit(0);
