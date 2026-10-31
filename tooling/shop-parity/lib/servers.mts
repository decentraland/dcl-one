// The three loopback servers of a capture: the reference build (static, SPA fallback), the replay
// that stands in for the node (sites SSR and its /shop-api proxy read it), and the built sites tree
// served by sites-preview.mts with that replay as its upstream.
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { createServer, type IncomingMessage, type Server } from "node:http";
import { extname, join, normalize, sep } from "node:path";
import { childEnv } from "./env.mts";
import { placeholderPng } from "./png.mts";
import { SITES, TOOL } from "./paths.mts";
import type { Recording } from "./recording.mts";
import { fromNodePath, isContentImage, OWN_IMAGE_MOUNTS, serviceForHost } from "./services.mts";

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".webmanifest": "application/manifest+json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".gif": "image/gif",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".txt": "text/plain; charset=utf-8",
  ".xml": "application/xml",
  ".mp4": "video/mp4",
  ".json5": "application/json",
  ".wasm": "application/wasm",
  ".glb": "model/gltf-binary",
};

function listen(server: Server, port: number): Promise<Server> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}

/** The reference build: files as built, every other path the SPA's index.html (vercel.json's rewrite). */
export function startStatic(dist: string, port: number): Promise<Server> {
  const server = createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    let path: string;
    try {
      path = decodeURIComponent(url.pathname);
    } catch {
      path = "/";
    }
    let file = normalize(join(dist, path));
    if (!file.startsWith(dist + sep) || !existsSync(file) || statSync(file).isDirectory()) file = join(dist, "index.html");
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream", "cache-control": "no-store" });
    res.end(readFileSync(file));
  });
  return listen(server, port);
}

function readBody(req: IncomingMessage): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

/** The picture a content URL stands for, independent of which host or mount serves it. */
export function imageSeed(url: URL): string {
  const p = decodeURIComponent(url.pathname);
  const urn = p.match(/(urn:decentraland:[^/]+)\/(thumbnail|image)$/i);
  if (urn) return `${urn[1]!.toLowerCase()}/${urn[2]}`;
  const content = p.match(/\/contents\/([A-Za-z0-9]{32,})$/);
  if (content) return content[1]!;
  // A profile picture, whether read from the profile-images service or through the node's mount.
  const face = p.match(/\/entities\/([A-Za-z0-9]{32,})\/(face256|face|body)(\.png)?$/);
  if (face) return `profile:${face[1]}/${face[2]}`;
  const loopback = url.hostname === "127.0.0.1" || url.hostname === "localhost";
  const service = loopback ? fromNodePath("GET", p).service : serviceForHost(url.host) ?? url.host;
  return `${service}:${loopback ? fromNodePath("GET", p).path : p}`;
}

export function placeholderFor(recording: Recording, url: URL): Buffer {
  const seed = imageSeed(url);
  const size = recording.images.get(seed);
  return placeholderPng(seed, size?.width, size?.height);
}

/**
 * A request a browser made: every current browser names the requesting site, and the sites preview
 * strips these headers when it relays.
 */
export function fromBrowser(headers: IncomingMessage["headers"]): boolean {
  return headers["sec-fetch-site"] !== undefined || headers.origin !== undefined;
}

/**
 * The node as sites sees it: every request answered from the recording, pictures by placeholder. Only
 * the sites preview reads it, server to server, so it answers no browser: while recording, a website
 * open in the developer's browser could otherwise make it read production and write the recording.
 * The preview names the replay by its loopback address, so any other Host is refused too: a page on
 * a name rebound to 127.0.0.1 already sends Sec-Fetch-Site, so this is a second lock, not the only one.
 */
export function startReplay(recording: Recording, port: number): Promise<Server> {
  const server = createServer(async (req, res) => {
    try {
      const url = new URL(req.url ?? "/", "http://127.0.0.1");
      const method = (req.method ?? "GET").toUpperCase();
      if (req.headers.host !== `127.0.0.1:${port}` || fromBrowser(req.headers) || !["GET", "HEAD", "POST"].includes(method)) {
        res.writeHead(403, { "content-type": "text/plain" });
        return res.end("the parity replay answers the sites preview only");
      }
      if (isContentImage(url) && OWN_IMAGE_MOUNTS.test(url.pathname)) {
        res.writeHead(200, { "content-type": "image/png" });
        return res.end(placeholderFor(recording, url));
      }
      const { service, path } = fromNodePath(method, url.pathname);
      const body = await readBody(req);
      const answer = await recording.answer({ method, service, path, search: url.search, body, headers: req.headers as Record<string, string>, side: "dcl-one" });
      res.writeHead(answer.status, { "content-type": answer.contentType });
      res.end(answer.body);
    } catch (err) {
      res.writeHead(502, { "content-type": "text/plain" });
      res.end(String(err));
    }
  });
  return listen(server, port);
}

/** The production preview of the built sites tree (sites-preview.mts), with the replay as its node. */
export async function startSites(port: number, upstream: string, env: Record<string, string> = {}): Promise<ChildProcess> {
  if (!existsSync(join(SITES, "build", "server", "index.js"))) throw new Error("apps/sites is not built: run `pnpm build` in apps/sites (or capture.mts --build)");
  const child = spawn(process.execPath, [join(TOOL, "sites-preview.mts"), "--base", `http://127.0.0.1:${port}`, "--upstream", upstream], {
    env: { ...childEnv(), CATALYST_URL: upstream, PUBLIC_CATALYST_URL: "", ...env },
    stdio: ["pipe", "pipe", "pipe"],
  });
  // Exiting without stopping the preview would leave its port taken for the next run.
  process.once("exit", () => child.kill());
  let log = "";
  child.stdout!.on("data", (c) => (log += c));
  child.stderr!.on("data", (c) => (log += c));
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) throw new Error(`sites preview exited (${child.exitCode ?? child.signalCode}):\n${log}`);
    // Only this child's own ready line counts: another server already on the port would answer too.
    if (!log.includes(`sites preview: http://127.0.0.1:${port}`)) {
      await new Promise((r) => setTimeout(r, 250));
      continue;
    }
    try {
      const r = await fetch(`http://127.0.0.1:${port}/robots.txt`, { signal: AbortSignal.timeout(2000) });
      await r.body?.cancel();
      if (r.status < 500) {
        // A later exit would otherwise surface only as refused connections on every page after it.
        child.once("exit", (code, signal) => {
          if (!child.killed) console.error(`sites preview exited (${code ?? signal}) during the run:\n${log.slice(-4000)}`);
        });
        return child;
      }
    } catch {
      // not listening yet
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  child.kill();
  throw new Error(`sites preview did not answer on ${port}:\n${log}`);
}
