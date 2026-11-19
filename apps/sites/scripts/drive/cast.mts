#!/usr/bin/env node
// Decentraland Cast end to end in a browser: the streamer page (`/cast/s/{key}`) publishes its
// camera and screen into a real LiveKit room, and the watcher page (`/cast/w/{location}`), signed
// in, receives them. A stand-in for the comms gatekeeper's `/cast/*` routes mints real LiveKit
// tokens with the contract `services/catalyst/comms/src/gatekeeper/streaming_cast.rs` serves; the
// gatekeeper's own decisions (keys, bans, world access) are covered by its Rust tests
// (`cast_tests.rs`). This drives the pages, `livekit-client` and the media path.
//
// Needs: this app's dependencies installed (and apps/ui3's), a `livekit-server` (LIVEKIT_SERVER_BIN,
// else on PATH, else `nix run nixpkgs#livekit`) and Chromium (as `smoke`: DCL_SHOT_CHROMIUM, else
// on PATH, else nix). Nothing is skipped: a missing dependency fails the run.
//
//   pnpm drive:cast [--out <dir>] [--keep]
//
// --out keeps the screenshots (default scripts/drive/out/cast); --keep leaves the servers running.
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { freePort, launchChromium, Tab } from "./cdp.mts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const APP = path.resolve(HERE, "../..");

const LIVEKIT_KEY = "castdrive";
const LIVEKIT_SECRET = crypto.randomBytes(24).toString("hex");
const ROOM = "scene:main:bafkreicastdrive";
const PLACE_ID = "cast-drive-place";
const STREAMING_KEY = "cast-drive-key";
const PARCEL = "10,10";
const PLACE_NAME = "Cast Drive Plaza";

function parseArgs(argv: string[]): { out: string; keep: boolean } {
  const opts = { out: path.join(HERE, "out", "cast"), keep: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--out") opts.out = path.resolve(argv[++i]!);
    else if (argv[i] === "--keep") opts.keep = true;
  }
  return opts;
}

const children: ChildProcess[] = [];
/** Children started in their own process group: stopping one also stops what `nix run`
 * started under it. They are killed outright: livekit-server drains its rooms on SIGTERM. */
const groups = new Set<ChildProcess>();

function stopAll(): void {
  for (const child of children) {
    try {
      if (groups.has(child) && child.pid) process.kill(-child.pid, "SIGKILL");
      else child.kill();
    } catch {
      // already gone
    }
  }
}

function startDetached(bin: string, args: string[], options: Parameters<typeof spawn>[2]): ChildProcess {
  const proc = spawn(bin, args, { ...options, detached: true });
  children.push(proc);
  groups.add(proc);
  return proc;
}

// ── LiveKit ──

function livekitCommand(): string[] {
  if (process.env.LIVEKIT_SERVER_BIN) return [process.env.LIVEKIT_SERVER_BIN];
  if (spawnSync("which", ["livekit-server"], { encoding: "utf8" }).status === 0) return ["livekit-server"];
  return ["nix", "run", "nixpkgs#livekit", "--"];
}

async function waitFor(what: string, probe: () => Promise<boolean>, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (await probe().catch(() => false)) return;
    if (Date.now() > deadline) throw new Error(`${what} did not come up in ${Math.round(timeoutMs / 1000)}s`);
    await new Promise((r) => setTimeout(r, 500));
  }
}

async function startLivekit(dir: string): Promise<string> {
  const port = await freePort();
  const tcp = await freePort();
  const udp = 40_000 + Math.floor(Math.random() * 20_000);
  const config = path.join(dir, "livekit.yaml");
  fs.writeFileSync(
    config,
    [
      `port: ${port}`,
      `bind_addresses: ["127.0.0.1"]`,
      "rtc:",
      `  tcp_port: ${tcp}`,
      `  port_range_start: ${udp}`,
      `  port_range_end: ${udp + 100}`,
      "  use_external_ip: false",
      "  node_ip: 127.0.0.1",
      "keys:",
      `  ${LIVEKIT_KEY}: ${LIVEKIT_SECRET}`,
      "logging:",
      "  level: warn",
      "",
    ].join("\n"),
  );
  const [bin, ...pre] = livekitCommand();
  const log = fs.openSync(path.join(dir, "livekit.log"), "w");
  const proc = startDetached(bin!, [...pre, "--config", config], { stdio: ["ignore", log, log] });
  proc.on("exit", (code) => {
    if (code) console.error(`livekit-server exited with ${code}; see ${path.join(dir, "livekit.log")}`);
  });
  await waitFor("livekit-server", async () => (await fetch(`http://127.0.0.1:${port}/`)).ok, 180_000);
  return `ws://127.0.0.1:${port}`;
}

// ── the gatekeeper stand-in ──

function jwt(identity: string, video: Record<string, unknown>, metadata: unknown, attributes?: Record<string, string>): string {
  const b64 = (o: unknown) => Buffer.from(JSON.stringify(o)).toString("base64url");
  const now = Math.floor(Date.now() / 1000);
  const claims: Record<string, unknown> = {
    iss: LIVEKIT_KEY,
    sub: identity,
    nbf: now - 5,
    exp: now + 3600,
    video,
    metadata: JSON.stringify(metadata),
  };
  if (attributes) claims.attributes = attributes;
  const head = `${b64({ alg: "HS256", typ: "JWT" })}.${b64(claims)}`;
  return `${head}.${crypto.createHmac("sha256", LIVEKIT_SECRET).update(head).digest("base64url")}`;
}

type Logged = { method: string; path: string; signed: boolean };

async function startGatekeeper(livekitUrl: string, log: Logged[]): Promise<string> {
  const port = await freePort();
  const accessId = crypto.randomUUID();
  const send = (res: http.ServerResponse, status: number, body: unknown) => {
    res.writeHead(status, { "content-type": "application/json" });
    res.end(JSON.stringify(body));
  };
  const server = http.createServer((req, res) => {
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", () => {
      const route = (req.url ?? "").split("?")[0]!.replace(/^\/comms-gatekeeper/, "");
      const signed = Boolean(req.headers["x-identity-auth-chain-0"]);
      log.push({ method: req.method ?? "", path: route, signed });
      const body = raw ? (JSON.parse(raw) as Record<string, string>) : {};
      if (req.method === "GET" && route.startsWith("/cast/stream-info/")) {
        return decodeURIComponent(route.slice("/cast/stream-info/".length)) === STREAMING_KEY
          ? send(res, 200, { placeName: PLACE_NAME, placeId: PLACE_ID, location: PARCEL, isWorld: false })
          : send(res, 400, { error: "Invalid streaming key" });
      }
      if (req.method === "POST" && route === "/cast/streamer-token") {
        if (body.token !== STREAMING_KEY) return send(res, 401, { error: "Invalid or expired streaming token" });
        const identity = `stream:${PLACE_ID}:${accessId}:${crypto.randomUUID()}`;
        const video = { room: ROOM, roomJoin: true, canPublish: true, canSubscribe: true, canPublishData: true, canUpdateOwnMetadata: true };
        return send(res, 200, {
          url: livekitUrl,
          roomId: ROOM,
          identity,
          token: jwt(identity, video, { role: "streamer", displayName: body.identity }),
        });
      }
      if (req.method === "POST" && route === "/cast/watcher-token") {
        // The real route verifies the viewer's signed fetch; here, that the page sent one.
        if (!signed) return send(res, 400, { ok: false, message: "Invalid Auth Chain" });
        if (body.location !== PARCEL) return send(res, 404, { error: `No active stream found for ${body.location}` });
        const identity = `watch:${ROOM}:${crypto.randomUUID()}`;
        const video = {
          room: ROOM,
          roomJoin: true,
          canPublish: false,
          canSubscribe: true,
          canPublishData: true,
          canUpdateOwnMetadata: false,
          canPublishSources: ["microphone"],
        };
        return send(res, 200, {
          url: livekitUrl,
          roomId: ROOM,
          identity,
          placeName: PLACE_NAME,
          token: jwt(identity, video, { role: "watcher", displayName: body.identity, roomId: ROOM }),
        });
      }
      send(res, 404, { error: "not found" });
    });
  });
  await new Promise<void>((resolve) => server.listen(port, "127.0.0.1", resolve));
  return `http://127.0.0.1:${port}`;
}

// ── the sites dev server ──

async function startSites(gatekeeper: string, dir: string): Promise<string> {
  const port = await freePort();
  const sync = spawnSync(process.execPath, ["scripts/sync-ui3-public.mts"], { cwd: APP, stdio: "inherit" });
  if (sync.status !== 0) throw new Error("scripts/sync-ui3-public.mts failed; are apps/ui3's dependencies installed?");
  const bin = path.join(APP, "node_modules/.bin/react-router");
  if (!fs.existsSync(bin)) throw new Error("apps/sites dependencies are not installed (pnpm install)");
  const log = fs.openSync(path.join(dir, "sites.log"), "w");
  startDetached(bin, ["dev", "--host", "127.0.0.1", "--port", String(port), "--strictPort"], {
    cwd: APP,
    env: { ...process.env, CATALYST_URL: gatekeeper, DCL_CATALYST_PROXY_TARGET: gatekeeper },
    stdio: ["ignore", log, log],
  });
  const base = `http://127.0.0.1:${port}`;
  await waitFor("the sites dev server", async () => (await fetch(`${base}/cast/s/${STREAMING_KEY}`)).ok, 240_000);
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

/** A page that renders as if it were the focused, visible tab. */
async function openPage(cdp: number): Promise<Tab> {
  const tab = await Tab.open(cdp);
  await tab.cmd("Emulation.setFocusEmulationEnabled", { enabled: true });
  await tab.cmd("Page.setWebLifecycleState", { state: "active" });
  return tab;
}

async function screenshot(tab: Tab, file: string): Promise<void> {
  fs.writeFileSync(file, Buffer.from(await tab.screenshotB64(), "base64"));
}

// ── the run ──

async function main(): Promise<number> {
  const opts = parseArgs(process.argv.slice(2));
  fs.mkdirSync(opts.out, { recursive: true });
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cast-drive-"));
  const log: Logged[] = [];
  const results: { name: string; ok: boolean; detail: string }[] = [];
  const check = (name: string, ok: boolean, detail = "") => {
    results.push({ name, ok, detail });
    console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` ${detail}` : ""}`);
  };

  const livekit = await startLivekit(dir);
  const gatekeeper = await startGatekeeper(livekit, log);
  const site = await startSites(gatekeeper, dir);
  console.log(`livekit ${livekit}, gatekeeper ${gatekeeper}, sites ${site}, logs in ${dir}`);

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
  await streamer.navigate(`${site}/cast/s/${STREAMING_KEY}`, 8_000);
  await streamer.navigate(`${site}/cast/s/${STREAMING_KEY}`, 3_000);
  const ready = await until(streamer, buttonExpr("JOIN NOW", controls), 120_000);
  check("the stream link resolves its place", ready && (await streamer.ev(`document.body.innerText.includes(${JSON.stringify(PLACE_NAME)})`)));
  await fill(streamer, "Your name", "Ada");
  await click(streamer, "JOIN NOW", controls);
  const joined = await until(streamer, buttonExpr("Share screen", controls), 30_000);
  const camera = joined && (await until(streamer, `[...document.querySelectorAll(".stcast__stage video")].some((v) => v.videoWidth > 0)`, 30_000));
  check("the streamer publishes its camera", camera, JSON.stringify(await streamer.ev(videos(".stcast__stage"))));
  await click(streamer, "Share screen", controls);
  const shared = await until(streamer, `document.querySelector('.stcast__stage video[data-source="screen_share"]')?.videoWidth > 0`, 30_000);
  check("the streamer shares its screen", shared, JSON.stringify(await streamer.ev(videos(".stcast__stage"))));
  await screenshot(streamer, path.join(opts.out, "streamer-live.png"));

  // A signed-in viewer: a throwaway session in the page's storage; the stand-in checks the
  // watcher token request is signed, the real route verifies the signature.
  const watcher = await openPage(cdp);
  const session = {
    signer: `0x${crypto.randomBytes(20).toString("hex")}`,
    ephemeral: {
      address: `0x${crypto.randomBytes(20).toString("hex")}`,
      privateKey: `0x${crypto.randomBytes(32).toString("hex")}`,
    },
    expiration: new Date(Date.now() + 3600e3).toISOString(),
    authChain: [{ type: "SIGNER", payload: "0x0", signature: "" }],
  };
  await watcher.cmd("Page.addScriptToEvaluateOnNewDocument", {
    source: `localStorage.setItem("dcl:auth:identity:v1", ${JSON.stringify(JSON.stringify(session))})`,
  });
  await watcher.navigate(`${site}/cast/w/${PARCEL}`, 3_000);
  await until(watcher, `document.querySelector("button.stcastwatcher__join")`, 90_000);
  await watcher.ev(`document.querySelector("button.stcastwatcher__join").click()`);
  const watching = await until(watcher, `document.querySelector(".stcastwatcher__stage video")?.videoWidth > 0`, 30_000);
  const seen = (await watcher.ev(videos(".stcastwatcher__stage"))) as { source?: string }[];
  check("the watcher sees the shared screen first", watching && seen[0]?.source === "screen_share", JSON.stringify(seen));
  check("the watcher plays the room's audio", await until(watcher, `document.querySelectorAll("audio").length > 0`, 10_000));
  check("the watcher page names the place from the token", await watcher.ev(`document.body.innerText.includes(${JSON.stringify(PLACE_NAME)})`));
  await screenshot(watcher, path.join(opts.out, "watcher-live.png"));

  await click(streamer, "Stop sharing", controls);
  check(
    "the watcher falls back to the camera when the share stops",
    await until(watcher, `document.querySelector(".stcastwatcher__stage video")?.dataset.source === "camera"`, 20_000),
    JSON.stringify(await watcher.ev(videos(".stcastwatcher__stage"))),
  );
  await click(streamer, "Turn camera off", controls);
  check("the streamer turns its camera off", await until(streamer, buttonExpr("Turn camera on", controls), 10_000));

  const empty = await openPage(cdp);
  await empty.cmd("Page.addScriptToEvaluateOnNewDocument", {
    source: `localStorage.setItem("dcl:auth:identity:v1", ${JSON.stringify(JSON.stringify(session))})`,
  });
  await empty.navigate(`${site}/cast/w/0,0`, 3_000);
  await until(empty, `document.querySelector("button.stcastwatcher__join")`, 90_000);
  await empty.ev(`document.querySelector("button.stcastwatcher__join").click()`);
  check("a place with no stream waits for one", await until(empty, `document.body.innerText.includes("No one is currently casting to this link")`, 20_000));

  const unknown = await openPage(cdp);
  await unknown.navigate(`${site}/cast/s/not-a-key`, 3_000);
  check("an unknown key shows the gatekeeper's reason", await until(unknown, `document.body.innerText.includes("Invalid streaming key")`, 60_000));
  await screenshot(unknown, path.join(opts.out, "streamer-invalid.png"));

  await click(streamer, "Leave stream", controls);
  check("leaving ends the cast", await until(streamer, `document.body.innerText.includes("Your cast has ended")`, 20_000));

  const watcherCalls = log.filter((l) => l.path === "/cast/watcher-token");
  check("watcher tokens are requested signed", watcherCalls.length > 0 && watcherCalls.every((l) => l.signed), JSON.stringify(watcherCalls));

  const failed = results.filter((r) => !r.ok).length;
  console.log(`${results.length - failed}/${results.length} passed; screenshots in ${opts.out}`);
  if (opts.keep) {
    console.log("--keep: servers left running; Ctrl-C stops them");
    await new Promise(() => {});
  }
  return failed === 0 ? 0 : 1;
}

process.on("SIGINT", () => {
  stopAll();
  process.exit(130);
});

main()
  .then((code) => {
    stopAll();
    process.exit(code);
  })
  .catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    stopAll();
    process.exit(1);
  });
