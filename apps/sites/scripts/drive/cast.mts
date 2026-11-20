#!/usr/bin/env node
// Decentraland Cast end to end against a real node: `catalyst serve` on a scratch database, its
// own livekit-server (with Valkey, where LiveKit keeps ingresses and its webhook pointed at the
// node) and `cast-bot` for presentations. A test wallet publishes its personal world
// (`<address>.dcl.eth`) through `POST /worlds-content-server/entities` and, as that world's owner,
// asks for a stream link the way the Admin Toolkit does: a scene-signed
// `GET /comms-gatekeeper/cast/generate-stream-link`, which creates the RTMP ingress its credential
// names (no livekit-ingress runs: nothing pushes RTMP). In a browser, the streamer page opens the
// returned link (`/cast/s/{key}`) and publishes its camera and screen into the world scene's
// LiveKit room; a second wallet, signed in, opens the watcher link (`/cast/w/{world}`) and
// receives them. Every stream-info, token and presenters answer comes from the node's gatekeeper,
// through this app's dev proxy as the gateway routes it.
//
// Presentation: with a presentation-bot token from `POST /comms-gatekeeper/cast/presentation-bot-token`
// a two-page PDF is posted to `POST /cast-presenter-service/presentations`; the node starts
// `cast-bot`, which publishes the slides as the `presentation` screen share. A presenter (a second
// streamer token, which the gatekeeper adds to the room's `presenters`) connects with the pages'
// `livekit-client` and sends `presentation:navigate` and `presentation:stop` as RFC-4 scene
// packets on the `presentation` topic (`services/catalyst/comms/src/cast_presenter/wire.rs`); the
// run checks the `presentation:state` and `presentation:stopped` broadcasts and the slide colour
// the watcher shows.
//
// Needs: this app's dependencies installed (and apps/ui3's); `CATALYST_TEST_DATABASE_URL`, a
// PostgreSQL the run creates (and drops) a scratch database on; the `catalyst` binary
// (CATALYST_BIN, else services/catalyst/target/debug/catalyst) and `cast-bot` (CAST_BOT_BIN, else
// services/catalyst/cast-bot/target/debug/cast-bot); `livekit-server` and `valkey-server`
// (LIVEKIT_SERVER_BIN, VALKEY_SERVER_BIN, else on PATH, else built from this flake's nixpkgs);
// Chromium (as `smoke`: DCL_SHOT_CHROMIUM, else on PATH, else nix). Nothing is skipped: a missing
// dependency fails the run.
//
//   pnpm drive:cast [--out <dir>] [--keep]
//
// --out keeps the screenshots (default scripts/drive/out/cast); --keep leaves the servers and the
// scratch database in place.
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import pg from "pg";
import { generatePrivateKey, privateKeyToAccount, type PrivateKeyAccount } from "viem/accounts";

import { freePort, launchChromium, Tab } from "./cdp.mts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const APP = path.resolve(HERE, "../..");
const ROOT = path.resolve(APP, "../..");

const LIVEKIT_KEY = "castdrive";
const LIVEKIT_SECRET = crypto.randomBytes(24).toString("hex");
const VALKEY_PASSWORD = crypto.randomBytes(16).toString("hex");
const SCENE_SIGNER = "decentraland-kernel-scene";
const WORLD_TITLE = "Cast Drive Plaza";

function parseArgs(argv: string[]): { out: string; keep: boolean } {
  const opts = { out: path.join(HERE, "out", "cast"), keep: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--out") opts.out = path.resolve(argv[++i]!);
    else if (argv[i] === "--keep") opts.keep = true;
  }
  return opts;
}

const children: ChildProcess[] = [];
const cleanups: (() => Promise<void>)[] = [];

function stopAll(): void {
  for (const child of children) {
    try {
      // The servers lead their own process groups: stopping one also stops what it started (the
      // node's cast-bot). Killed outright: livekit-server drains its rooms on SIGTERM.
      if (child.pid) process.kill(-child.pid, "SIGKILL");
    } catch {
      // not a group leader (Chromium), or already gone
      child.kill("SIGKILL");
    }
  }
}

function start(name: string, bin: string, args: string[], dir: string, env: NodeJS.ProcessEnv): ChildProcess {
  const log = fs.openSync(path.join(dir, `${name}.log`), "w");
  const proc = spawn(bin, args, { env, stdio: ["ignore", log, log], detached: true });
  children.push(proc);
  proc.on("exit", (code) => {
    if (code) console.error(`${name} exited with ${code}; see ${path.join(dir, `${name}.log`)}`);
  });
  return proc;
}

async function waitFor(what: string, probe: () => Promise<boolean>, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (await probe().catch(() => false)) return;
    if (Date.now() > deadline) throw new Error(`${what} did not come up in ${Math.round(timeoutMs / 1000)}s`);
    await new Promise((r) => setTimeout(r, 500));
  }
}

// ── binaries ──

/** `envVar`, else `name` on PATH, else `nix build` of this flake's nixpkgs attribute. */
function tool(envVar: string, name: string, attr: string): string {
  const set = process.env[envVar];
  if (set) {
    if (!fs.existsSync(set)) throw new Error(`${envVar}=${set} does not exist`);
    return set;
  }
  const which = spawnSync("which", [name], { encoding: "utf8" });
  if (which.status === 0) return which.stdout.trim();
  console.log(`building nixpkgs#${attr} for ${name} (set ${envVar} to skip)`);
  const built = spawnSync("nix", ["build", "--inputs-from", ROOT, `nixpkgs#${attr}`, "--no-link", "--print-out-paths"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  const out = built.stdout?.trim().split("\n").find((p) => fs.existsSync(path.join(p, "bin", name)));
  if (built.status !== 0 || !out) throw new Error(`${name} not found: set ${envVar}, put it on PATH or install Nix`);
  return path.join(out, "bin", name);
}

function builtBinary(envVar: string, fallback: string, build: string): string {
  const bin = process.env[envVar] || path.join(ROOT, fallback);
  if (!fs.existsSync(bin)) throw new Error(`${bin} is missing: set ${envVar} or build it (${build})`);
  return bin;
}

// ── the scratch database ──

async function scratchDatabase(): Promise<{ url: string; name: string; drop: () => Promise<void> }> {
  const admin = process.env.CATALYST_TEST_DATABASE_URL;
  if (!admin) throw new Error("CATALYST_TEST_DATABASE_URL is not set (a PostgreSQL the run creates a scratch database on)");
  const name = `cast_drive_${crypto.randomBytes(5).toString("hex")}`;
  const client = new pg.Client({ connectionString: admin });
  await client.connect();
  await client.query(`CREATE DATABASE ${name}`);
  await client.end();
  const url = new URL(admin);
  url.pathname = `/${name}`;
  // The schema's collation, as tooling/dev.py ensures it: Darwin's libc names it en_US.UTF-8.
  const db = new pg.Client({ connectionString: url.toString() });
  await db.connect();
  const { rows } = await db.query(
    "SELECT 1 FROM pg_collation WHERE collname = 'en_US.utf8' AND collnamespace = 'pg_catalog'::regnamespace",
  );
  if (rows.length === 0) await db.query(`CREATE COLLATION pg_catalog."en_US.utf8" (provider = libc, locale = 'en_US.UTF-8')`);
  await db.end();
  return {
    url: url.toString(),
    name,
    drop: async () => {
      const c = new pg.Client({ connectionString: admin });
      await c.connect();
      await c.query(`DROP DATABASE IF EXISTS ${name} WITH (FORCE)`);
      await c.end();
    },
  };
}

// ── LiveKit and Valkey ──

type Ports = {
  node: number;
  metrics: number;
  site: number;
  livekit: number;
  rtcTcp: number;
  rtcUdp: number;
  valkey: number;
  rtmp: number;
};

function listening(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const s = net.connect(port, "127.0.0.1", () => {
      s.end();
      resolve(true);
    });
    s.on("error", () => resolve(false));
  });
}

async function startMedia(dir: string, ports: Ports): Promise<string> {
  const env = { PATH: process.env.PATH, HOME: process.env.HOME, TMPDIR: process.env.TMPDIR };
  const valkeyConf = path.join(dir, "valkey.conf");
  fs.writeFileSync(valkeyConf, `bind 127.0.0.1\nport ${ports.valkey}\nrequirepass ${VALKEY_PASSWORD}\nsave ""\nappendonly no\n`, {
    mode: 0o600,
  });
  start("valkey", tool("VALKEY_SERVER_BIN", "valkey-server", "valkey"), [valkeyConf], dir, env);
  await waitFor("valkey", () => listening(ports.valkey), 60_000);

  // As tooling/dev.py: the webhook drives the node's Cast presenter bookkeeping; LiveKit keeps its
  // ingresses in Valkey, so `generate-stream-link` creates the RTMP ingress its credential names.
  // No livekit-ingress process: nothing here pushes RTMP.
  const livekitConf = path.join(dir, "livekit.yaml");
  fs.writeFileSync(
    livekitConf,
    [
      `port: ${ports.livekit}`,
      `bind_addresses: ["127.0.0.1"]`,
      "rtc:",
      `  tcp_port: ${ports.rtcTcp}`,
      `  port_range_start: ${ports.rtcUdp}`,
      `  port_range_end: ${ports.rtcUdp + 100}`,
      "  use_external_ip: false",
      "  node_ip: 127.0.0.1",
      "keys:",
      `  ${LIVEKIT_KEY}: ${LIVEKIT_SECRET}`,
      "webhook:",
      `  api_key: ${LIVEKIT_KEY}`,
      "  urls:",
      `    - http://127.0.0.1:${ports.node}/comms-gatekeeper/livekit-webhook`,
      "redis:",
      `  address: 127.0.0.1:${ports.valkey}`,
      `  password: ${VALKEY_PASSWORD}`,
      "ingress:",
      `  rtmp_base_url: rtmp://127.0.0.1:${ports.rtmp}/x`,
      "logging:",
      "  level: warn",
      "",
    ].join("\n"),
  );
  start("livekit", tool("LIVEKIT_SERVER_BIN", "livekit-server", "livekit"), ["--config", livekitConf], dir, env);
  await waitFor("livekit-server", async () => (await fetch(`http://127.0.0.1:${ports.livekit}/`)).ok, 180_000);

  return `ws://127.0.0.1:${ports.livekit}`;
}

// ── the node ──

async function startNode(dir: string, ports: Ports, databaseUrl: string, livekit: string): Promise<string> {
  const catalyst = builtBinary("CATALYST_BIN", "services/catalyst/target/debug/catalyst", "cd services/catalyst && cargo build --bin catalyst");
  const castBot = builtBinary("CAST_BOT_BIN", "services/catalyst/cast-bot/target/debug/cast-bot", "cd services/catalyst/cast-bot && cargo build");
  const base = `http://127.0.0.1:${ports.node}`;
  const env: NodeJS.ProcessEnv = {
    PATH: process.env.PATH,
    HOME: process.env.HOME,
    TMPDIR: process.env.TMPDIR,
    RUST_LOG: "info",
    DATABASE_URL: databaseUrl,
    HTTP_SERVER_HOST: "127.0.0.1",
    HTTP_SERVER_PORT: String(ports.node),
    SERVE_METRICS_BIND: `127.0.0.1:${ports.metrics}`,
    PUBLIC_URL: base,
    REALM_NAME: "cast-drive",
    SERVE_UNSYNCED: "true",
    SYNC_ENABLED: "false",
    STORAGE_ROOT_FOLDER: path.join(dir, "content"),
    PROFILE_IMAGES_CACHE_DIR: path.join(dir, "profile-images"),
    AB_CDN_CACHE_DIR: path.join(dir, "ab-cdn"),
    BVIMPOSTERS_STORE_ROOT: path.join(dir, "bvimposters"),
    ENCRYPTION_KEY: crypto.randomBytes(32).toString("hex"),
    WORLDS_WRITES_ENABLED: "true",
    // The Cast tokens are social writes (presenters in the room metadata).
    SOCIAL_WRITES_ENABLED: "true",
    WORLDS_PERSONAL_WORLDS_MAX: "5",
    SCENE_HOST_ENABLED: "false",
    // A published world becomes a place (`explore.place_world_local`) on the places-derive job's
    // next pass; the stream link needs that place.
    PLACES_DERIVE_INTERVAL_SECS: "2",
    LIVEKIT_HOST: livekit,
    LIVEKIT_API_KEY: LIVEKIT_KEY,
    LIVEKIT_API_SECRET: LIVEKIT_SECRET,
    CAST_BOT_BIN: castBot,
    // The stream and watcher links name this app's pages.
    CAST2_URL: `http://127.0.0.1:${ports.site}/cast`,
  };
  for (const sub of ["content", "profile-images", "ab-cdn", "bvimposters"]) fs.mkdirSync(path.join(dir, sub), { recursive: true });
  const schema = spawnSync(catalyst, ["schema", "apply"], { env, encoding: "utf8" });
  if (schema.status !== 0) throw new Error(`catalyst schema apply failed:\n${schema.stderr}`);
  start("catalyst", catalyst, ["serve"], dir, env);
  await waitFor("catalyst serve", async () => (await fetch(`${base}/about`)).ok, 120_000);
  return base;
}

// ── wallets, signed fetches and the personal world ──

type Wallet = {
  address: string;
  ephemeral: PrivateKeyAccount;
  ephemeralKey: `0x${string}`;
  mandate: string;
  mandateSig: string;
  expiration: string;
};

async function newWallet(): Promise<Wallet> {
  const account = privateKeyToAccount(generatePrivateKey());
  const ephemeralKey = generatePrivateKey();
  const ephemeral = privateKeyToAccount(ephemeralKey);
  const expiration = new Date(Date.now() + 24 * 3600e3).toISOString();
  const mandate = ["Decentraland Login", `Ephemeral address: ${ephemeral.address.toLowerCase()}`, `Expiration: ${expiration}`].join("\n");
  return {
    address: account.address.toLowerCase(),
    ephemeral,
    ephemeralKey,
    mandate,
    mandateSig: await account.signMessage({ message: mandate }),
    expiration,
  };
}

function identityChain(w: Wallet) {
  return [
    { type: "SIGNER", payload: w.address, signature: "" },
    { type: "ECDSA_EPHEMERAL", payload: w.mandate, signature: w.mandateSig },
  ];
}

/** The explorer's chain over `payload`: SIGNER, the mandate, the payload signed by the ephemeral key. */
async function chain(w: Wallet, payload: string) {
  return [
    ...identityChain(w),
    { type: "ECDSA_SIGNED_ENTITY", payload, signature: await w.ephemeral.signMessage({ message: payload }) },
  ];
}

/** The session a signed-in page keeps in `dcl:auth:identity:v1`. */
function session(w: Wallet) {
  return {
    signer: w.address,
    ephemeral: { address: w.ephemeral.address.toLowerCase(), privateKey: w.ephemeralKey, publicKey: w.ephemeral.publicKey },
    expiration: w.expiration,
    authChain: identityChain(w),
  };
}

async function signedFetch(w: Wallet, method: string, url: string, metadata: Record<string, unknown>): Promise<Response> {
  const pathname = new URL(url).pathname;
  const timestamp = String(Date.now());
  const meta = JSON.stringify(metadata);
  const headers: Record<string, string> = { "x-identity-timestamp": timestamp, "x-identity-metadata": meta };
  (await chain(w, `${method}:${pathname}:${timestamp}:${meta}`.toLowerCase())).forEach((link, i) => {
    headers[`x-identity-auth-chain-${i}`] = JSON.stringify(link);
  });
  return fetch(url, { method, headers });
}

/** A content hash as the node computes it: CIDv1, raw codec, sha2-256, base32. */
function cid(bytes: Uint8Array): string {
  const digest = crypto.createHash("sha256").update(bytes).digest();
  const raw = Buffer.concat([Buffer.from([0x01, 0x55, 0x12, 0x20]), digest]);
  const alphabet = "abcdefghijklmnopqrstuvwxyz234567";
  let bits = 0;
  let value = 0;
  let out = "b";
  for (const byte of raw) {
    value = ((value << 8) | byte) & 0xffff;
    bits += 8;
    while (bits >= 5) {
      out += alphabet[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += alphabet[(value << (5 - bits)) & 31];
  return out;
}

/** Publishes `<address>.dcl.eth` with a one-file SDK7 scene; returns the world and the entity id. */
async function publishPersonalWorld(node: string, w: Wallet, title: string): Promise<{ world: string; entityId: string }> {
  const world = `${w.address}.dcl.eth`;
  const game = Buffer.from("export function onUpdate() {}\n");
  const gameHash = cid(game);
  const entity = Buffer.from(
    JSON.stringify({
      version: "v3",
      type: "scene",
      timestamp: Date.now(),
      pointers: ["0,0"],
      content: [{ file: "bin/game.js", hash: gameHash }],
      metadata: {
        display: { title },
        main: "bin/game.js",
        runtimeVersion: "7",
        worldConfiguration: { name: world },
        scene: { base: "0,0", parcels: ["0,0"] },
      },
    }),
  );
  const entityId = cid(entity);
  const form = new FormData();
  form.append("entityId", entityId);
  (await chain(w, entityId)).forEach((link, i) => {
    for (const k of ["type", "payload", "signature"] as const) form.append(`authChain[${i}][${k}]`, link[k]);
  });
  form.append(entityId, new Blob([new Uint8Array(entity)]), entityId);
  form.append(gameHash, new Blob([new Uint8Array(game)]), gameHash);
  const res = await fetch(`${node}/worlds-content-server/entities`, { method: "POST", body: form });
  if (!res.ok) throw new Error(`publishing ${world} answered ${res.status}: ${await res.text()}`);
  return { world, entityId };
}

// ── the presentation ──

/** A PDF of 400x300 pages, each a coloured panel on white (blue, then red). */
function slidesPdf(): Buffer {
  const colours = ["0.1 0.3 0.9", "0.9 0.15 0.1"];
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    `<< /Type /Pages /Kids [${colours.map((_, i) => `${3 + i * 2} 0 R`).join(" ")}] /Count ${colours.length} >>`,
  ];
  colours.forEach((rgb, i) => {
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Contents ${4 + i * 2} 0 R >>`);
    const stream = `${rgb} rg 40 40 320 220 re f`;
    objects.push(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`);
  });
  let out = "%PDF-1.4\n";
  const offsets: number[] = [];
  objects.forEach((body, i) => {
    offsets.push(out.length);
    out += `${i + 1} 0 obj\n${body}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const o of offsets) out += `${String(o).padStart(10, "0")} 00000 n \n`;
  out += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(out, "latin1");
}

/** In a page: the RFC-4 framing of `cast_presenter/wire.rs`. `Packet { scene(6) { scene_id(1),
 * data(2) }, protocol_version(11) = 100 }`, data `[3][topic length u16 LE][topic][json]`. */
const PAGE_CODEC = `
window.__castCodec = (() => {
  const enc = new TextEncoder(), dec = new TextDecoder();
  const varint = (n) => { const out = []; while (n > 127) { out.push((n & 127) | 128); n >>>= 7; } out.push(n); return out; };
  const field = (tag, bytes) => [tag, ...varint(bytes.length), ...bytes];
  const encode = (sceneId, message) => {
    const topic = enc.encode("presentation"), json = enc.encode(JSON.stringify(message));
    const data = [3, topic.length & 255, topic.length >> 8, ...topic, ...json];
    const scene = [...field(0x0a, enc.encode(sceneId)), ...field(0x12, data)];
    return new Uint8Array([...field(0x32, scene), 0x58, 100]);
  };
  const read = (buf) => {
    const fields = []; let i = 0;
    const v = () => { let n = 0, s = 0, b; do { b = buf[i++]; n |= (b & 127) << s; s += 7; } while (b & 128); return n >>> 0; };
    while (i < buf.length) {
      const key = v(), no = key >>> 3, wt = key & 7;
      if (wt === 0) fields.push([no, v()]);
      else if (wt === 2) { const len = v(); fields.push([no, buf.subarray(i, i + len)]); i += len; }
      else return null;
    }
    return fields;
  };
  const decode = (payload) => {
    try {
      const scene = read(payload)?.find(([no]) => no === 6)?.[1];
      const data = scene && read(scene)?.find(([no]) => no === 2)?.[1];
      if (!data || data[0] !== 3) return null;
      const len = data[1] | (data[2] << 8);
      if (dec.decode(data.subarray(3, 3 + len)) !== "presentation") return null;
      return JSON.parse(dec.decode(data.subarray(3 + len)));
    } catch { return null; }
  };
  return { encode, decode };
})(); true`;

// ── the sites dev server ──

async function startSites(node: string, port: number, dir: string): Promise<string> {
  const sync = spawnSync(process.execPath, ["scripts/sync-ui3-public.mts"], { cwd: APP, stdio: "inherit" });
  if (sync.status !== 0) throw new Error("scripts/sync-ui3-public.mts failed; are apps/ui3's dependencies installed?");
  const bin = path.join(APP, "node_modules/.bin/react-router");
  if (!fs.existsSync(bin)) throw new Error("apps/sites dependencies are not installed (pnpm install)");
  const log = fs.openSync(path.join(dir, "sites.log"), "w");
  const proc = spawn(bin, ["dev", "--host", "127.0.0.1", "--port", String(port), "--strictPort"], {
    cwd: APP,
    env: { ...process.env, CATALYST_URL: node, DCL_CATALYST_PROXY_TARGET: node },
    stdio: ["ignore", log, log],
    detached: true,
  });
  children.push(proc);
  const base = `http://127.0.0.1:${port}`;
  await waitFor("the sites dev server", async () => (await fetch(`${base}/cast/s/not-a-key`)).ok, 240_000);
  return base;
}

// ── page helpers ──

/** A button by its accessible name (aria-label, else text), inside `scope`. */
const buttonExpr = (name: string, scope = "body") =>
  `[...document.querySelectorAll(${JSON.stringify(`${scope} button`)})].find((b) => (b.getAttribute("aria-label") || b.textContent || "").trim() === ${JSON.stringify(name)})`;

async function until(tab: Tab, expr: string, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (await tab.ev(`Boolean(${expr})`).catch(() => false)) return true;
    if (Date.now() > deadline) return false;
    await new Promise((r) => setTimeout(r, 300));
  }
}

async function click(tab: Tab, name: string, scope?: string, timeoutMs = 30_000): Promise<boolean> {
  const expr = buttonExpr(name, scope);
  if (!(await until(tab, `(${expr}) && !(${expr}).disabled`, timeoutMs))) return false;
  await tab.ev(`(${expr}).click()`);
  return true;
}

/** Types into the input labelled `label` the way React sees a user type. */
async function fill(tab: Tab, label: string, value: string): Promise<void> {
  await tab.ev(`(() => {
    const input = [...document.querySelectorAll("input")].find((i) => i.getAttribute("aria-label") === ${JSON.stringify(label)});
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    set.call(input, ${JSON.stringify(value)});
    input.dispatchEvent(new Event("input", { bubbles: true }));
  })()`);
}

const videos = (scope: string) =>
  `[...document.querySelectorAll(${JSON.stringify(`${scope} video`)})].map((v) => ({ source: v.dataset.source, w: v.videoWidth, h: v.videoHeight, track: v.srcObject?.getVideoTracks().map((t) => ({ state: t.readyState, muted: t.muted, enabled: t.enabled, size: [t.getSettings().width, t.getSettings().height] })) }))`;

/** The watcher's stage video and the colour at its centre. */
const STAGE_PIXEL = `(() => {
  const v = document.querySelector(".stcastwatcher__stage video");
  if (!v || !v.videoWidth) return null;
  const c = document.createElement("canvas");
  c.width = v.videoWidth; c.height = v.videoHeight;
  const g = c.getContext("2d");
  g.drawImage(v, 0, 0);
  const d = g.getImageData(c.width >> 1, c.height >> 1, 1, 1).data;
  return { source: v.dataset.source, w: v.videoWidth, h: v.videoHeight, rgb: [d[0], d[1], d[2]] };
})()`;
const BLUE_SLIDE = `((p) => p && p.source === "screen_share" && p.rgb[2] > 150 && p.rgb[0] < 100)(${STAGE_PIXEL})`;
const RED_SLIDE = `((p) => p && p.source === "screen_share" && p.rgb[0] > 150 && p.rgb[2] < 100)(${STAGE_PIXEL})`;

/** A page that renders as if it were the focused, visible tab. */
async function openPage(cdp: number): Promise<Tab> {
  const tab = await Tab.open(cdp);
  await tab.cmd("Emulation.setFocusEmulationEnabled", { enabled: true });
  await tab.cmd("Page.setWebLifecycleState", { state: "active" });
  // Vite serves hundreds of modules: keep every resource entry, so the run finds the pages'
  // own livekit-client module among them.
  await tab.cmd("Page.addScriptToEvaluateOnNewDocument", { source: "performance.setResourceTimingBufferSize(100000)" });
  return tab;
}

async function signIn(tab: Tab, w: Wallet): Promise<void> {
  await tab.cmd("Page.addScriptToEvaluateOnNewDocument", {
    source: `localStorage.setItem("dcl:auth:identity:v1", ${JSON.stringify(JSON.stringify(session(w)))})`,
  });
}

async function screenshot(tab: Tab, file: string): Promise<void> {
  fs.writeFileSync(file, Buffer.from(await tab.screenshotB64(), "base64"));
}

async function body(res: Response): Promise<any> {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text };
  }
}

function postJson(url: string, value: unknown): Promise<Response> {
  return fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(value) });
}

// ── the run ──

async function main(opts: { out: string; keep: boolean }): Promise<number> {
  fs.mkdirSync(opts.out, { recursive: true });
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cast-drive-"));
  const results: { name: string; ok: boolean; detail: string }[] = [];
  const check = (name: string, ok: boolean, detail = "") => {
    results.push({ name, ok, detail });
    console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` ${detail}` : ""}`);
    return ok;
  };

  const ports: Ports = {
    node: await freePort(),
    metrics: await freePort(),
    site: await freePort(),
    livekit: await freePort(),
    rtcTcp: await freePort(),
    rtcUdp: 40_000 + Math.floor(Math.random() * 20_000),
    valkey: await freePort(),
    rtmp: await freePort(),
  };
  const db = await scratchDatabase();
  if (!opts.keep) cleanups.push(db.drop);
  const livekit = await startMedia(dir, ports);
  const node = await startNode(dir, ports, db.url, livekit);
  const site = await startSites(node, ports.site, dir);
  console.log(`node ${node}, livekit ${livekit}, sites ${site}, database ${db.name}, logs in ${dir}`);

  // The world's owner publishes it and asks for a stream link as the Admin Toolkit does: a
  // scene-signed fetch naming the scene and its realm.
  const owner = await newWallet();
  const viewer = await newWallet();
  const { world, entityId } = await publishPersonalWorld(node, owner, WORLD_TITLE);
  const { world: quietWorld } = await publishPersonalWorld(node, viewer, "Nobody casts here");
  const sceneMetadata = {
    signer: SCENE_SIGNER,
    sceneId: entityId,
    parcel: "0,0",
    realm: { serverName: world, hostname: node, protocol: "v3" },
  };
  // Until the places-derive pass lists the world, the link answers 404 "place not found".
  let linkRes = await signedFetch(owner, "GET", `${node}/comms-gatekeeper/cast/generate-stream-link`, sceneMetadata);
  for (const deadline = Date.now() + 60_000; linkRes.status === 404 && Date.now() < deadline; ) {
    await new Promise((r) => setTimeout(r, 1_000));
    linkRes = await signedFetch(owner, "GET", `${node}/comms-gatekeeper/cast/generate-stream-link`, sceneMetadata);
  }
  const link = await body(linkRes);
  if (!check("the world's owner generates a stream link", linkRes.ok && typeof link.streamingKey === "string", JSON.stringify(link))) {
    return 1;
  }
  const streamingKey: string = link.streamingKey;
  check(
    "the links name this app's stream and watch pages",
    link.streamLink === `${site}/cast/s/${streamingKey}` && link.watcherLink === `${site}/cast/w/${world}`,
    `${link.streamLink} ${link.watcherLink}`,
  );
  const stranger = await newWallet();
  const refused = await signedFetch(stranger, "GET", `${node}/comms-gatekeeper/cast/generate-stream-link`, sceneMetadata);
  check("a wallet that does not own the world gets no stream link", refused.status === 403, `${refused.status} ${JSON.stringify(await body(refused))}`);

  const cdp = await freePort();
  const chromium = await launchChromium({
    port: cdp,
    profileDir: path.join(dir, "chromium"),
    gpu: true,
    extraArgs: [
      "--use-fake-ui-for-media-stream",
      "--use-fake-device-for-media-stream",
      "--auto-select-desktop-capture-source=Entire screen",
      "--autoplay-policy=no-user-gesture-required",
      // Several pages play video at once: none of them is a throttled background tab.
      "--disable-background-timer-throttling",
      "--disable-renderer-backgrounding",
      "--disable-backgrounding-occluded-windows",
    ],
  });
  children.push(chromium);

  const controls = ".stcast";
  const streamer = await openPage(cdp);
  // The first load lets Vite optimize the pages' dependencies, which reloads the page once.
  await streamer.navigate(link.streamLink, 8_000);
  await streamer.navigate(link.streamLink, 3_000);
  const ready = await until(streamer, buttonExpr("JOIN NOW", controls), 120_000);
  check("the stream link resolves its world", ready && (await streamer.ev(`document.body.innerText.includes(${JSON.stringify(world)})`)));
  await fill(streamer, "Your name", "Ada");
  await click(streamer, "JOIN NOW", controls);
  const joined = await until(streamer, buttonExpr("Share screen", controls), 30_000);
  const camera = joined && (await until(streamer, `[...document.querySelectorAll(".stcast__stage video")].some((v) => v.videoWidth > 0)`, 30_000));
  check("the streamer publishes its camera", camera, JSON.stringify(await streamer.ev(videos(".stcast__stage"))));
  await click(streamer, "Share screen", controls);
  const shared = await until(streamer, `document.querySelector('.stcast__stage video[data-source="screen_share"]')?.videoWidth > 0`, 30_000);
  check("the streamer shares its screen", shared, JSON.stringify(await streamer.ev(videos(".stcast__stage"))));
  await screenshot(streamer, path.join(opts.out, "streamer-live.png"));

  const presentersRes = await signedFetch(owner, "GET", `${node}/comms-gatekeeper/cast/presenters`, sceneMetadata);
  const presenters = await body(presentersRes);
  check(
    "the streamer is one of the room's presenters",
    presentersRes.ok && (presenters.presenters ?? []).some((id: string) => id.startsWith("stream:")),
    JSON.stringify(presenters),
  );

  // A signed-in viewer with a real session: the node verifies the watcher token's signed fetch.
  const unsigned = await postJson(`${node}/comms-gatekeeper/cast/watcher-token`, { location: world, identity: "Eve" });
  check("an unsigned watcher token request is refused", unsigned.status >= 400 && unsigned.status < 500, String(unsigned.status));
  const watcher = await openPage(cdp);
  await signIn(watcher, viewer);
  await watcher.navigate(link.watcherLink, 3_000);
  await until(watcher, `document.querySelector("button.stcastwatcher__join")`, 90_000);
  await watcher.ev(`document.querySelector("button.stcastwatcher__join").click()`);
  const watching = await until(watcher, `document.querySelector(".stcastwatcher__stage video")?.videoWidth > 0`, 30_000);
  const seen = (await watcher.ev(videos(".stcastwatcher__stage"))) as { source?: string }[];
  check("the watcher sees the shared screen first", watching && seen[0]?.source === "screen_share", JSON.stringify(seen));
  check("the watcher plays the room's audio", await until(watcher, `document.querySelectorAll("audio").length > 0`, 10_000));
  check("the watcher page names the place from the token", await watcher.ev(`document.body.innerText.includes(${JSON.stringify(WORLD_TITLE)})`));
  await screenshot(watcher, path.join(opts.out, "watcher-live.png"));

  await click(streamer, "Stop sharing", controls);
  check(
    "the watcher falls back to the camera when the share stops",
    await until(watcher, `document.querySelector(".stcastwatcher__stage video")?.dataset.source === "camera"`, 20_000),
    JSON.stringify(await watcher.ev(videos(".stcastwatcher__stage"))),
  );

  // ── the presentation ──
  const botRes = await postJson(`${node}/comms-gatekeeper/cast/presentation-bot-token`, { streamingKey });
  const bot = await body(botRes);
  check("the stream key gets a presentation-bot token", botRes.ok && typeof bot.token === "string", `${botRes.status} ${bot.roomId ?? JSON.stringify(bot)}`);
  const upload = new FormData();
  upload.append("file", new Blob([new Uint8Array(slidesPdf())], { type: "application/pdf" }), "slides.pdf");
  upload.append("livekitToken", bot.token);
  upload.append("livekitUrl", bot.url);
  const created = await fetch(`${node}/cast-presenter-service/presentations`, { method: "POST", body: upload });
  const presentation = await body(created);
  check(
    "the presenter service starts the PDF presentation",
    created.status === 201 && presentation.slideCount === 2 && presentation.currentSlide === 0,
    `${created.status} ${JSON.stringify(presentation)}`,
  );
  check("the watcher shows the presentation's first slide", await until(watcher, BLUE_SLIDE, 45_000), JSON.stringify(await watcher.ev(STAGE_PIXEL)));
  await screenshot(watcher, path.join(opts.out, "watcher-presentation.png"));

  // A presenter: another streamer of the key, which the gatekeeper adds to `presenters`. It
  // connects with the pages' own livekit-client and speaks the scene packet framing.
  const presenterRes = await postJson(`${node}/comms-gatekeeper/cast/streamer-token`, { token: streamingKey, identity: "Presenter" });
  const presenter = await body(presenterRes);
  const livekitModule = (await watcher.ev(
    `performance.getEntriesByType("resource").map((e) => e.name).find((n) => /\\/livekit-client\\.js/.test(n)) ?? null`,
  )) as string | null;
  if (!check("a presenter token and the pages' livekit-client module", presenterRes.ok && Boolean(livekitModule), `${presenterRes.status} ${livekitModule}`)) {
    return 1;
  }
  const control = await openPage(cdp);
  await control.navigate(`${site}/cast/s/not-a-key`, 3_000);
  await control.ev(PAGE_CODEC);
  await control.ev(
    `(async () => {
      const lk = await import(${JSON.stringify(livekitModule)});
      const room = new lk.Room();
      window.__messages = [];
      room.on(lk.RoomEvent.DataReceived, (payload, participant) => {
        const message = window.__castCodec.decode(payload);
        if (message) window.__messages.push({ from: participant?.identity ?? null, ...message });
      });
      await room.connect(${JSON.stringify(presenter.url)}, ${JSON.stringify(presenter.token)});
      window.__room = room;
      return true;
    })()`,
    { awaitPromise: true },
  );
  const botTrack = `[...window.__room.remoteParticipants.values()].flatMap((p) => [...p.trackPublications.values()].map((t) => ({ identity: p.identity, name: t.trackName, source: t.source }))).find((t) => t.identity.startsWith("presentation-bot:") && t.name === "presentation" && t.source === "screen_share")`;
  check("the bot publishes the `presentation` screen share", await until(control, botTrack, 20_000), JSON.stringify(await control.ev(`(${botTrack}) ?? null`)));
  const send = (message: Record<string, unknown>) =>
    control.ev(
      `window.__room.localParticipant.publishData(window.__castCodec.encode(${JSON.stringify(entityId)}, ${JSON.stringify(message)}), { reliable: true }).then(() => true)`,
      { awaitPromise: true },
    );
  // The bot obeys the identities the room's metadata lists.
  await until(control, `window.__room.metadata?.includes(window.__room.localParticipant.identity)`, 10_000);
  await send({ type: "presentation:navigate", action: "next" });
  const advanced = `window.__messages.find((m) => m.type === "presentation:state" && m.currentSlide === 1 && m.slideCount === 2 && m.id === ${JSON.stringify(presentation.id)})`;
  check("`navigate next` broadcasts a state on the next slide", await until(control, advanced, 15_000), JSON.stringify(await control.ev(`window.__messages`)));
  check("the watcher shows the second slide", await until(watcher, RED_SLIDE, 20_000), JSON.stringify(await watcher.ev(STAGE_PIXEL)));
  await screenshot(watcher, path.join(opts.out, "watcher-slide-2.png"));
  await send({ type: "presentation:stop" });
  check(
    "`stop` broadcasts presentation:stopped",
    await until(control, `window.__messages.find((m) => m.type === "presentation:stopped" && m.id === ${JSON.stringify(presentation.id)})`, 15_000),
    JSON.stringify(await control.ev(`window.__messages.slice(-2)`)),
  );
  check(
    "the bot leaves the room",
    await until(control, `![...window.__room.remoteParticipants.keys()].some((id) => id.startsWith("presentation-bot:"))`, 20_000),
  );
  await control.ev(`window.__room.disconnect().then(() => true)`, { awaitPromise: true });

  await click(streamer, "Turn camera off", controls);
  check("the streamer turns its camera off", await until(streamer, buttonExpr("Turn camera on", controls), 10_000));

  const empty = await openPage(cdp);
  await signIn(empty, viewer);
  await empty.navigate(`${site}/cast/w/${quietWorld}`, 3_000);
  await until(empty, `document.querySelector("button.stcastwatcher__join")`, 90_000);
  await empty.ev(`document.querySelector("button.stcastwatcher__join").click()`);
  check("a world with no stream waits for one", await until(empty, `document.body.innerText.includes("No one is currently casting to this link")`, 20_000));

  const unknown = await openPage(cdp);
  await unknown.navigate(`${site}/cast/s/not-a-key`, 3_000);
  check("an unknown key shows the gatekeeper's reason", await until(unknown, `document.body.innerText.includes("Invalid streaming key")`, 60_000));
  await screenshot(unknown, path.join(opts.out, "streamer-invalid.png"));

  await click(streamer, "Leave stream", controls);
  check("leaving ends the cast", await until(streamer, `document.body.innerText.includes("Your cast has ended")`, 20_000));

  const failed = results.filter((r) => !r.ok).length;
  console.log(`${results.length - failed}/${results.length} passed; screenshots in ${opts.out}`);
  if (opts.keep) {
    console.log("--keep: servers and the scratch database left in place; Ctrl-C stops the servers");
    await new Promise(() => {});
  }
  return failed === 0 ? 0 : 1;
}

async function finish(code: number): Promise<never> {
  stopAll();
  for (const cleanup of cleanups) await cleanup().catch((e) => console.error(`cleanup: ${e instanceof Error ? e.message : e}`));
  process.exit(code);
}

process.on("SIGINT", () => void finish(130));

main(parseArgs(process.argv.slice(2)))
  .then(finish)
  .catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    return finish(1);
  });
