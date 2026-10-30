// Serve the built apps/sites tree in production mode on a loopback origin, with every API mount the
// browser reads proxied to one upstream (the capture's replay).
//
//   node sites-preview.mts --base http://127.0.0.1:<port> --upstream http://127.0.0.1:<port>
import { createRequire } from "node:module";
import { request } from "node:http";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { SITES } from "./lib/paths.mts";

// Every path production routes off the sites app, so a page's read reaches the replay (and is listed as a
// miss when the recording lacks it) instead of rendering the app's 404 page.
const MOUNTS = ["lambdas", "content", "builder-items", "auth-api", "builder-api", "shop-api", "marketplace-api", "transactions-api", "signatures-api", "rpc", "camera-reel", "media", "profile-images", "telemetry", "credits", "api/events", "api/places", "api/destinations", "api/schedules", "api/map"];

const arg = (name: string): string => {
  const i = process.argv.indexOf(name);
  if (i < 0 || !process.argv[i + 1]) throw new Error(`${name} is required`);
  return process.argv[i + 1]!;
};
const base = new URL(arg("--base"));
const upstream = new URL(arg("--upstream"));
for (const url of [base, upstream]) if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || !url.port) throw new Error(`use an explicit HTTP loopback origin, not ${url.href}`);

// The browser's clock is fixed at the recording's time; so is the server-side render's, or anything it
// computes from the date drifts as the recording ages and between the two sides of one run.
const now = Number(process.env.SHOP_PARITY_NOW);
if (now) {
  const RealDate = Date;
  globalThis.Date = new Proxy(RealDate, {
    construct: (target, args, newTarget) => Reflect.construct(target, args.length ? args : [now], newTarget),
    apply: () => new RealDate(now).toString(),
    get: (target, key, receiver) => (key === "now" ? () => now : Reflect.get(target, key, receiver)),
  });
}
// The same seeded sequence the browser gets (lib/browser.mts), restarted for every request as the browser's
// is for every page, so a server-side shuffle does not depend on what rendered before it.
const seed = Number(process.env.SHOP_PARITY_SEED);
let reseed = () => {};
if (process.env.SHOP_PARITY_SEED && Number.isInteger(seed)) {
  let s = seed >>> 0;
  reseed = () => {
    s = seed >>> 0;
  };
  Math.random = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

process.env.NODE_ENV = "production";
process.env.CATALYST_URL = upstream.origin;
process.chdir(SITES);
const serverRequire = createRequire(createRequire(resolve(SITES, "package.json")).resolve("@react-router/serve/package.json"));
const express = serverRequire("express");
const compression = serverRequire("compression");
const { createRequestHandler } = serverRequire("@react-router/express");
const build = await import(pathToFileURL(resolve(SITES, "build/server/index.js")).href);

const app = express();
app.disable("x-powered-by");
app.use(compression());
app.use((_incoming: unknown, _outgoing: unknown, next: () => void) => {
  reseed();
  next();
});
app.use((incoming: any, outgoing: any, next: () => void) => {
  if (!MOUNTS.some((m) => incoming.path === `/${m}` || incoming.path.startsWith(`/${m}/`))) return next();
  const target = new URL(incoming.url, upstream);
  const req = request(target, { method: incoming.method, headers: { ...incoming.headers, host: target.host, "accept-encoding": "identity" } }, (res) => {
    outgoing.status(res.statusCode ?? 502);
    for (const [key, value] of Object.entries(res.headers)) if (value && !["transfer-encoding", "connection"].includes(key)) outgoing.setHeader(key, value);
    res.on("error", () => outgoing.destroy());
    res.pipe(outgoing);
  });
  req.setTimeout(30_000, () => req.destroy(new Error("upstream timeout")));
  req.on("error", () => (outgoing.headersSent ? outgoing.destroy() : outgoing.status(502).end()));
  outgoing.on("close", () => req.destroy());
  incoming.pipe(req);
});
app.use("/assets", express.static(resolve(SITES, "build/client/assets"), { immutable: true, maxAge: "1y" }));
app.use(express.static(resolve(SITES, "build/client")));
app.use(express.static(resolve(SITES, "public"), { maxAge: "1h" }));
app.all("/{*splat}", createRequestHandler({ build, mode: "production" }));
const fail = (error: Error) => {
  console.error(`sites preview failed: ${error.message}`);
  process.exit(1);
};
const server = app.listen(Number(base.port), base.hostname, (error?: Error) => (error ? fail(error) : console.log(`sites preview: ${base.origin}`)));
server.on("error", fail);
for (const signal of ["SIGINT", "SIGTERM"] as const) process.once(signal, () => process.exit(0));
// The capture that started this preview owns it: stop when its pipe closes.
process.stdin.on("end", () => process.exit(0)).resume();
