import { execFileSync } from 'child_process';
import { createHash } from 'crypto';
import fs from 'fs';
import { resolve } from 'path';

import { AGENT_BASE_URL, REALM_NAME, SERVED_DIR } from '../bevy-agent-realm.js';

// Export the super-user editor-agent scene (agents/bevy) as a STATIC realm into
// the inspector's served `public/bevy-agent/` dir. The Bevy engine loads the
// agent via `?systemScene=<realm url>`, and it consumes a realm the Catalyst way
// — it GETs `<systemScene>/about`, whose `scenesUrn` carries the entity + a
// `baseUrl` the content hashes resolve against. `sdk-commands export-static`
// produces exactly that layout (a `<realmName>/about` file + content-hash files),
// so a plain static host (our static server) can serve it — no second sdk-commands
// process at runtime.
//
// The one runtime-dynamic bit is the origin: the serving port is chosen per
// launch, so the baseUrl carries a placeholder that the server swaps out per
// request — the Creator Hub's inspector server in the app, the dev proxy in
// build.js when running against a watch build. Everything else is
// content-addressed and immutable. See ../bevy-agent-realm.js for the contract.
//
// The agent is a SEPARATE SDK7 project with its own node_modules (NOT a
// workspace) pinned to the engine's companion SDK — so we run ITS sdk-commands,
// not the inspector's, both to build it and to export it.

const AGENT_DIR = resolve(import.meta.dirname, '../agents/bevy');
const DEST = resolve(import.meta.dirname, '..', 'public', SERVED_DIR);
const AGENT_SOURCES = [
  resolve(AGENT_DIR, 'src'),
  resolve(import.meta.dirname, '../agents/protocol/src'),
];

const SDK_COMMANDS_DIR = resolve(AGENT_DIR, 'node_modules/@dcl/sdk-commands');
const SDK_COMMANDS_MANIFEST = resolve(SDK_COMMANDS_DIR, 'package.json');
if (!fs.existsSync(SDK_COMMANDS_MANIFEST)) {
  throw new Error(
    `Agent has no sdk-commands: ${SDK_COMMANDS_MANIFEST} is missing. Run \`pnpm install\` in agents/bevy first.`,
  );
}
const sdkCommandsPackage = JSON.parse(fs.readFileSync(SDK_COMMANDS_MANIFEST, 'utf8'));
// The JavaScript entry of the CLI, started with this Node. The shim in `node_modules/.bin` is a
// shell script, and `execFile` cannot start it on Windows.
const sdkCommands = resolve(SDK_COMMANDS_DIR, sdkCommandsPackage.bin['sdk-commands']);
// The build sends no usage event. The CLI holds each event for about ten seconds before it sends
// it, and the process stays open for that time.
const SDK_COMMANDS_ENV = { ...process.env, DCL_DISABLE_ANALYTICS: 'true' };

/** Newest mtime under `dir`, recursively (directory entries included — harmless for a max). */
function newestMtime(dir: string): number {
  return fs
    .readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .reduce((newest, entry) => Math.max(newest, fs.statSync(resolve(dir, entry)).mtimeMs), 0);
}

/**
 * Build the agent when its sources are newer than its bundle (or it has none).
 *
 * Without this the export ships a STALE agent: the inspector's own build/watch
 * only COPIES `bin/index.js`, so an agent source change stays invisible at
 * runtime — the engine keeps loading the old bundle — until someone remembers
 * `make build-bevy-agent`.
 */
function buildAgentIfStale(): void {
  const agentBin = resolve(AGENT_DIR, 'bin/index.js');
  const binMtime = fs.existsSync(agentBin) ? fs.statSync(agentBin).mtimeMs : 0;
  if (!AGENT_SOURCES.some(dir => newestMtime(dir) > binMtime)) return;
  console.log('Agent sources changed → building agents/bevy...');
  execFileSync(process.execPath, [sdkCommands, 'build'], {
    cwd: AGENT_DIR,
    stdio: 'inherit',
    env: SDK_COMMANDS_ENV,
  });
}

buildAgentIfStale();

const about = resolve(DEST, REALM_NAME, 'about');
// Outside `public/`, so the stamp is neither served nor published.
const STAMP = resolve(import.meta.dirname, '../node_modules/.cache/bevy-agent-export.sha256');

/** Every file under `dir` as a sorted relative path, without the top-level folder `skip`. */
function listFiles(dir: string, skip?: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .filter(entry => entry.split(/[\\/]/)[0] !== skip && fs.statSync(resolve(dir, entry)).isFile())
    .sort();
}

/**
 * A digest of what the export reads (the agent project, the installed sdk-commands, the realm
 * settings) and of the files it wrote. An equal digest means a new export would differ only in
 * the timestamp of the entity.
 */
function exportStamp(): string {
  const hash = createHash('sha256');
  hash.update(JSON.stringify([REALM_NAME, AGENT_BASE_URL, sdkCommandsPackage.version]));
  for (const file of listFiles(AGENT_DIR, 'node_modules')) {
    hash.update(`\0${file}\0`).update(fs.readFileSync(resolve(AGENT_DIR, file)));
  }
  for (const file of listFiles(DEST)) {
    hash.update(`\0${file}\0${fs.statSync(resolve(DEST, file)).size}`);
  }
  return hash.digest('hex');
}

function exportAgent(): void {
  console.log('Exporting agent scene → static realm at "public/bevy-agent"...');

  // Fresh each export so a changed agent can't leave stale content-hash files behind.
  fs.rmSync(DEST, { recursive: true, force: true });
  fs.mkdirSync(DEST, { recursive: true });

  execFileSync(
    process.execPath,
    [
      sdkCommands,
      'export-static',
      '--dir',
      AGENT_DIR,
      '--destination',
      DEST,
      '--realmName',
      REALM_NAME,
      '--baseUrl',
      AGENT_BASE_URL,
    ],
    { cwd: AGENT_DIR, stdio: 'inherit', env: SDK_COMMANDS_ENV },
  );

  if (!fs.existsSync(about)) {
    throw new Error(`export-static did not produce ${about}`);
  }

  fs.mkdirSync(resolve(STAMP, '..'), { recursive: true });
  fs.writeFileSync(STAMP, exportStamp());
  console.log(`Exported agent realm → ${DEST} (systemScene = <origin>/bevy-agent/bevy-agent)`);
}

// The export takes several seconds, so a build with no agent change keeps the last one.
if (
  fs.existsSync(about) &&
  fs.existsSync(STAMP) &&
  fs.readFileSync(STAMP, 'utf8') === exportStamp()
) {
  console.log('Agent realm is current: kept "public/bevy-agent".');
} else {
  exportAgent();
}
