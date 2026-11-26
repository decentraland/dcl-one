#!/usr/bin/env node
// Publishes a scene to the local stack as the personal world of a throwaway owner, so its signed
// features (the admin tools, Cast and Stream) work outside preview, and optionally drives the web
// Explorer in it as that owner through local-harness/web-driver. docs/development.md, "A scene as
// its owner", has the setup.
//
//   pnpm drive:scene-world --scene <dir> [--checkout-sdk] [--drive] [--owner <file>]
//
// --checkout-sdk builds a copy of the scene with this checkout's SDK (library/js-sdk-toolchain)
// instead of the one it installed. --drive opens /play in the world, signs in as the owner and runs
// the commands on stdin, one per line: console commands, `sleep <seconds>`, `screenshot <file>` and
// `log`. Each prints one JSON line.
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";

import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "../../../..");
const STATE = path.join(ROOT, ".cache/dev");
const SDK_PACKAGES = path.join(ROOT, "library/js-sdk-toolchain/packages/@dcl");
const DRIVER = path.join(ROOT, "rendering/bevy-explorer/source/local-harness/web-driver/driver.mjs");
const SCENE_SIGNER = "decentraland-kernel-scene";
// What a deploy leaves out besides the scene's own .dclignore.
const ALWAYS_SKIPPED = [/(^|\/)\./, /(^|\/)node_modules(\/|$)/, /\.tsx?$/, /\.map$/, /^src\//];

type Options = { scene: string; checkoutSdk: boolean; drive: boolean; owner: string };
type Owner = { address: string; ephemeralKey: `0x${string}`; mandate: string; mandateSig: string; expiration: string };
type Link = { type: string; payload: string; signature: string };
type Driver = {
  ready(o: { timeout: number }): Promise<void>;
  console(line: string): Promise<string>;
  screenshot(): Promise<Buffer>;
  consoleMessages(): unknown[];
  close(): Promise<void>;
};

function parseArgs(argv: string[]): Options {
  const opts: Options = { scene: "", checkoutSdk: false, drive: false, owner: path.join(STATE, "scene-owner.json") };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--scene") opts.scene = path.resolve(argv[++i] ?? "");
    else if (a === "--owner") opts.owner = path.resolve(argv[++i] ?? "");
    else if (a === "--checkout-sdk") opts.checkoutSdk = true;
    else if (a === "--drive") opts.drive = true;
    else throw new Error(`unknown argument ${a}`);
  }
  if (!opts.scene || !fs.existsSync(path.join(opts.scene, "scene.json"))) throw new Error("--scene must name a folder with a scene.json");
  return opts;
}

/** The stack's API and Sites origins, from the ports `foundation-dev` saved. */
function stack(): { node: string; site: string } {
  const settings = JSON.parse(fs.readFileSync(path.join(STATE, "settings.json"), "utf8"));
  const base = Number(settings.port ?? 5210);
  if (!settings.personal_worlds) {
    throw new Error('the stack hosts no personal worlds: set "personal_worlds": 5 in .cache/dev/settings.json, then foundation-dev down and up');
  }
  return { node: `http://127.0.0.1:${base}`, site: `http://127.0.0.1:${base + 2}` };
}

/** The owner kept in `file` while its sign-in lasts another day, else a new throwaway one. */
async function owner(file: string): Promise<Owner> {
  if (fs.existsSync(file)) {
    const kept = JSON.parse(fs.readFileSync(file, "utf8")) as Owner;
    if (Date.parse(kept.expiration) > Date.now() + 24 * 3600e3) return kept;
  }
  const account = privateKeyToAccount(generatePrivateKey());
  const ephemeralKey = generatePrivateKey();
  const ephemeral = privateKeyToAccount(ephemeralKey);
  const expiration = new Date(Date.now() + 7 * 24 * 3600e3).toISOString();
  const mandate = ["Decentraland Login", `Ephemeral address: ${ephemeral.address.toLowerCase()}`, `Expiration: ${expiration}`].join("\n");
  const created = { address: account.address.toLowerCase(), ephemeralKey, mandate, mandateSig: await account.signMessage({ message: mandate }), expiration };
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(created), { mode: 0o600 });
  return created;
}

async function chain(o: Owner, payload: string): Promise<Link[]> {
  return [
    { type: "SIGNER", payload: o.address, signature: "" },
    { type: "ECDSA_EPHEMERAL", payload: o.mandate, signature: o.mandateSig },
    { type: "ECDSA_SIGNED_ENTITY", payload, signature: await privateKeyToAccount(o.ephemeralKey).signMessage({ message: payload }) },
  ];
}

/** The identity `/hud_login` takes, base64 of its JSON. */
function identity(o: Owner): string {
  const ephemeral = privateKeyToAccount(o.ephemeralKey);
  return Buffer.from(
    JSON.stringify({
      ephemeralIdentity: { address: ephemeral.address.toLowerCase(), privateKey: o.ephemeralKey, publicKey: ephemeral.publicKey },
      expiration: o.expiration,
      authChain: [
        { type: "SIGNER", payload: o.address, signature: "" },
        { type: "ECDSA_EPHEMERAL", payload: o.mandate, signature: o.mandateSig },
      ],
    }),
  ).toString("base64");
}

/** A copy of the scene under .cache/dev built with this checkout's SDK packages. */
function buildWithCheckoutSdk(scene: string): string {
  for (const name of ["ecs", "sdk", "js-runtime", "react-ecs"]) {
    const manifest = path.join(SDK_PACKAGES, name, "package.json");
    const entry = fs.existsSync(manifest) ? (JSON.parse(fs.readFileSync(manifest, "utf8")) as { main?: string; types?: string }) : null;
    if (!entry || !fs.existsSync(path.join(SDK_PACKAGES, name, entry.main ?? entry.types ?? "index.d.ts"))) {
      throw new Error("build the checkout's SDK first: (cd library/js-sdk-toolchain && make install build)");
    }
  }
  if (!fs.existsSync(path.join(scene, "node_modules/@dcl/sdk"))) throw new Error(`install the scene's packages first: (cd ${scene} && npm install)`);
  const copy = path.join(STATE, "scene-world", path.basename(scene));
  fs.rmSync(copy, { recursive: true, force: true });
  fs.cpSync(scene, copy, { recursive: true });
  for (const name of ["ecs", "sdk", "js-runtime", "react-ecs"]) {
    const target = path.join(copy, "node_modules/@dcl", name);
    fs.rmSync(target, { recursive: true, force: true });
    fs.cpSync(path.join(SDK_PACKAGES, name), target, { recursive: true, filter: (src) => !/(^|\/)(node_modules|src)(\/|$)/.test(path.relative(path.join(SDK_PACKAGES, name), src)) });
  }
  const built = spawnSync("npx", ["sdk-commands", "build"], { cwd: copy, stdio: "inherit" });
  if (built.status !== 0) throw new Error("the scene did not build with the checkout's SDK");
  return copy;
}

function ignored(scene: string): RegExp[] {
  const file = path.join(scene, ".dclignore");
  const lines = fs.existsSync(file) ? fs.readFileSync(file, "utf8").split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#")) : [];
  const globs = lines.map((line) => {
    const body = line.replace(/\/$/, "").replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, "[^/]*").replace(/\?/g, "[^/]");
    return new RegExp(line.includes("/") && !line.endsWith("/") ? `^${body}$` : `(^|/)${body}(/|$)`);
  });
  return [...ALWAYS_SKIPPED, ...globs];
}

function files(root: string, skip: RegExp[], rel = ""): string[] {
  return fs.readdirSync(path.join(root, rel), { withFileTypes: true }).flatMap((entry) => {
    const file = rel ? `${rel}/${entry.name}` : entry.name;
    if (skip.some((pattern) => pattern.test(file))) return [];
    return entry.isDirectory() ? files(root, skip, file) : [file];
  });
}

/** Deploys the scene as `<owner>.dcl.eth`, hashed with the scene's own @dcl/hashing. */
async function publish(scene: string, node: string, o: Owner): Promise<{ world: string; entityId: string; base: string }> {
  const hashing = createRequire(path.join(scene, "package.json")).resolve("@dcl/hashing");
  const { hashV1 } = (await import(pathToFileURL(hashing).href)) as { hashV1: (b: Uint8Array) => Promise<string> };
  const world = `${o.address}.dcl.eth`;
  const metadata = JSON.parse(fs.readFileSync(path.join(scene, "scene.json"), "utf8"));
  metadata.worldConfiguration = { ...(metadata.worldConfiguration ?? {}), name: world };
  if (!fs.existsSync(path.join(scene, metadata.main ?? "bin/index.js"))) throw new Error(`build the scene first: ${metadata.main ?? "bin/index.js"} is missing`);
  const blobs = new Map<string, Buffer>();
  const content: { file: string; hash: string }[] = [];
  for (const file of files(scene, ignored(scene))) {
    const bytes = file === "scene.json" ? Buffer.from(JSON.stringify(metadata)) : fs.readFileSync(path.join(scene, file));
    const hash = await hashV1(bytes);
    blobs.set(hash, bytes);
    content.push({ file, hash });
  }
  const pointers: string[] = metadata.scene.parcels;
  const entity = Buffer.from(JSON.stringify({ version: "v3", type: "scene", timestamp: Date.now(), pointers, content, metadata }));
  const entityId = await hashV1(entity);
  const form = new FormData();
  form.append("entityId", entityId);
  (await chain(o, entityId)).forEach((link, i) => {
    for (const key of ["type", "payload", "signature"] as const) form.append(`authChain[${i}][${key}]`, link[key]);
  });
  form.append(entityId, new Blob([new Uint8Array(entity)]), entityId);
  for (const [hash, bytes] of blobs) form.append(hash, new Blob([new Uint8Array(bytes)]), hash);
  const res = await fetch(`${node}/worlds-content-server/entities`, { method: "POST", body: form });
  if (!res.ok) throw new Error(`publishing ${world} answered ${res.status}: ${await res.text()}`);
  return { world, entityId, base: metadata.scene.base };
}

/** Waits until the gatekeeper names the owner an admin of the new scene, as the admin tools ask it. */
async function adminReady(node: string, o: Owner, published: { world: string; entityId: string; base: string }): Promise<boolean> {
  const route = "/comms-gatekeeper/scene-admin";
  const meta = JSON.stringify({ signer: SCENE_SIGNER, sceneId: published.entityId, parcel: published.base, realm: { serverName: published.world, hostname: node, protocol: "v3" } });
  for (const deadline = Date.now() + 90_000; Date.now() < deadline; await new Promise((r) => setTimeout(r, 2_000))) {
    const timestamp = String(Date.now());
    const headers: Record<string, string> = { "x-identity-timestamp": timestamp, "x-identity-metadata": meta };
    (await chain(o, `get:${route}:${timestamp}:${meta}`.toLowerCase())).forEach((link, i) => (headers[`x-identity-auth-chain-${i}`] = JSON.stringify(link)));
    if ((await fetch(node + route, { headers })).ok) return true;
  }
  return false;
}

function chromeArgs(): string[] {
  const args = ["--enable-unsafe-webgpu", "--autoplay-policy=no-user-gesture-required", "--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"];
  const key = path.join(STATE, "pulse/key.pem");
  const der = fs.existsSync(key) ? spawnSync("openssl", ["pkey", "-in", key, "-pubout", "-outform", "DER"]) : null;
  if (der?.status === 0) {
    // Pulse's self-signed certificate, trusted the way `foundation-dev status` tells Chrome to.
    const spki = crypto.createHash("sha256").update(der.stdout).digest("base64");
    args.push("--webtransport-developer-mode", `--ignore-certificate-errors-spki-list=${spki}`);
  }
  return args;
}

/** `/hud_login` as the owner, tried again while the HUD or the engine is still starting. */
async function signIn(driver: Driver, o: Owner, published: { world: string; base: string }): Promise<string> {
  for (const deadline = Date.now() + 120_000; ; await new Promise((r) => setTimeout(r, 2_000))) {
    try {
      return await driver.console(`/hud_login ${identity(o)} --name SceneOwner --look 1 --realm ${published.world} --position ${published.base}`);
    } catch (error) {
      if (!/HUD is not mounted|not ready to take a sign-in/.test(String((error as Error).message)) || Date.now() > deadline) throw error;
    }
  }
}

async function drive(play: string, o: Owner, published: { world: string; base: string }): Promise<number> {
  const { launch } = (await import(pathToFileURL(DRIVER).href)) as { launch: (o: object) => Promise<Driver> };
  const driver = await launch({ url: play, args: chromeArgs(), windowSize: [1600, 1000] });
  let failed = false;
  const say = (value: object) => process.stdout.write(`${JSON.stringify(value)}\n`);
  try {
    await driver.ready({ timeout: 240_000 });
    say({ command: "/hud_login <owner>", ok: true, output: await signIn(driver, o, published) });
    for await (const raw of readline.createInterface({ input: process.stdin })) {
      const line = raw.trim();
      if (!line) continue;
      try {
        if (line.startsWith("sleep ")) await new Promise((r) => setTimeout(r, Number(line.slice(6)) * 1000));
        else if (line.startsWith("screenshot ")) fs.writeFileSync(path.resolve(line.slice(11)), await driver.screenshot());
        else if (line === "log") {
          say({ command: line, ok: true, output: driver.consoleMessages().slice(-50) });
          continue;
        } else {
          say({ command: line, ok: true, output: await driver.console(line) });
          continue;
        }
        say({ command: line, ok: true });
      } catch (error) {
        failed = true;
        say({ command: line, ok: false, error: String((error as Error).message ?? error) });
      }
    }
  } finally {
    await driver.close();
  }
  return failed ? 1 : 0;
}

async function main(): Promise<number> {
  const opts = parseArgs(process.argv.slice(2));
  const { node, site } = stack();
  const o = await owner(opts.owner);
  const scene = opts.checkoutSdk ? buildWithCheckoutSdk(opts.scene) : opts.scene;
  const published = await publish(scene, node, o);
  const ready = await adminReady(node, o, published);
  const play = `${site}/play?world=${published.world}&position=${published.base}`;
  // A page opened before this deploy keeps the old scene id, which the admin check refuses: open a new one.
  process.stderr.write(`${JSON.stringify({ world: published.world, entityId: published.entityId, play, adminReady: ready, owner: opts.owner })}\n`);
  if (!ready) process.stderr.write("the gatekeeper does not name the owner an admin yet: the admin tools stay hidden\n");
  return opts.drive ? await drive(play, o, published) : 0;
}

main().then(
  (code) => process.exit(code),
  (error) => {
    process.stderr.write(`scene-world: ${(error as Error).message}\n`);
    process.exit(2);
  },
);
