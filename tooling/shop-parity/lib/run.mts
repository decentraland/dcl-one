// One capture run: the servers up, every page × viewport captured on both sides with the same
// recording, compared structurally and by pixels, and written under the run directory.
import type { ChildProcess } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import type { Server } from "node:http";
import { join } from "node:path";
import type { Browser, Page } from "playwright-core";
import { pagePath, type PageSpec, type Side } from "../pages.mts";
import { launch, VIEWPORTS, type Viewport } from "./browser.mts";
import type { Box } from "./collect.mts";
import { captureSide, type SideCapture } from "./capture.mts";
import { PORTS, referenceDist } from "./paths.mts";
import { decodePng, encodePng } from "./png.mts";
import { pixelDiff, type Band } from "./pixeldiff.mts";
import type { Miss, Recording } from "./recording.mts";
import { startReplay, startSites, startStatic } from "./servers.mts";
import { compareStructure, coverByRegions, line, type StructDiff } from "./structure.mts";
import { classifyMiss, classifyUnknown, isIdentical } from "./verdict.mts";

export const SEED = 7;

export type PageResult = {
  page: string;
  title: string;
  viewport: string;
  path: string;
  identical: boolean;
  pixels: { diff: number; antialiased: number; masked: number; ratio: number; width: number; height: number; bands: Band[] };
  structure: StructDiff[];
  lines: string[];
  masks: { why: string; reference: Box[]; dclOne: Box[] }[];
  regions: { why: string; reference: Box[]; dclOne: Box[] }[];
  sides: Record<Side, { url: string; error?: string; console: string[]; blocked: string[]; unknown: string[]; height: number }>;
  /** Requests the recording could not answer; `expected` names the page's rule when one covers it. */
  misses: (Miss & { expected?: string })[];
  /** Requests to hosts the harness does not know (answered 404); `expected` as for misses. */
  unknown: { side: Side; request: string; expected?: string }[];
};

export type Servers = {
  origins: Record<Side, string>;
  /** Hosts dcl-one's server-side render was refused since the last call, once its queued renders end. */
  ssrBlocked(): Promise<string[]>;
  close(): Promise<void>;
};

/** `now` (epoch ms) fixes the server-side render's clock where the browser's is; its randomness is seeded as the browser's. */
export async function startServers(recording: Recording, now?: number): Promise<Servers> {
  const servers: Server[] = [];
  let sites: ChildProcess | null = null;
  try {
    servers.push(await startStatic(referenceDist(), PORTS.reference()));
    servers.push(await startReplay(recording, PORTS.replay()));
    sites = await startSites(PORTS.sites(), `http://127.0.0.1:${PORTS.replay()}`, { SHOP_PARITY_SEED: String(SEED), ...(now === undefined ? {} : { SHOP_PARITY_NOW: String(now) }) });
  } catch (err) {
    for (const s of servers) s.close();
    throw err;
  }
  return {
    origins: { reference: `http://127.0.0.1:${PORTS.reference()}`, "dcl-one": `http://127.0.0.1:${PORTS.sites()}` },
    async ssrBlocked() {
      const r = await fetch(`http://127.0.0.1:${PORTS.sites()}/__shop-parity/blocked`, { signal: AbortSignal.timeout(60_000) });
      return (await r.json()) as string[];
    },
    async close() {
      sites?.kill();
      await Promise.all(servers.map((s) => new Promise((r) => s.close(() => r(null)))));
    },
  };
}

export async function comparePage(
  browser: Browser,
  recording: Recording,
  servers: Servers,
  spec: PageSpec,
  vp: Viewport,
  opts: { now: number; targets: Record<string, string>; out: string; onPage?: (side: Side, page: Page) => Promise<void> },
): Promise<PageResult> {
  const missesBefore = recording.misses.length;
  const caps = {} as Record<Side, SideCapture>;
  for (const side of ["reference", "dcl-one"] as const) {
    caps[side] = await captureSide(browser, spec, vp, side, {
      origin: servers.origins[side],
      recording,
      now: opts.now,
      seed: SEED,
      targets: opts.targets,
      onPage: opts.onPage ? (p) => opts.onPage!(side, p) : undefined,
    });
  }
  // Asked after dcl-one's context is closed and its last render has ended, so no host lands on the next page.
  const ssrBlocked = await servers.ssrBlocked();
  const dir = join(opts.out, `${spec.id}@${vp.name}`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "reference.png"), caps.reference.png);
  writeFileSync(join(dir, "dcl-one.png"), caps["dcl-one"].png);
  writeFileSync(join(dir, "structure.json"), JSON.stringify({ reference: caps.reference.collected, "dcl-one": caps["dcl-one"].collected }));

  const masks = (spec.masks ?? []).map((m, i) => ({ why: m.why, reference: caps.reference.maskGroups[i] ?? [], dclOne: caps["dcl-one"].maskGroups[i] ?? [] }));
  const regions = (spec.regions ?? []).map((m, i) => ({ why: m.why, reference: caps.reference.regionGroups[i] ?? [], dclOne: caps["dcl-one"].regionGroups[i] ?? [] }));
  let pixels: PageResult["pixels"] = { diff: Number.POSITIVE_INFINITY, antialiased: 0, masked: 0, ratio: 1, width: 0, height: 0, bands: [] };
  try {
    // A mask hides a box only where both sides have one; a region hides its area on both sides.
    const p = pixelDiff(decodePng(caps.reference.png), decodePng(caps["dcl-one"].png), {
      masks: { a: caps.reference.masks, b: caps["dcl-one"].masks },
      regions: [...caps.reference.regions, ...caps["dcl-one"].regions],
    });
    writeFileSync(join(dir, "diff.png"), encodePng(p.image));
    pixels = { diff: p.diff, antialiased: p.antialiased, masked: p.masked, ratio: p.ratio, width: p.width, height: p.height, bands: p.bands.slice(0, 12) };
  } catch (err) {
    caps.reference.error ??= `pixel diff: ${String(err)}`;
  }
  const structure = compareStructure(caps.reference.collected, caps["dcl-one"].collected, { tolerance: 1, expected: spec.expected });
  coverByRegions(structure, spec.regions ?? [], caps.reference.regionGroups, caps["dcl-one"].regionGroups);
  const sides = Object.fromEntries(
    (["reference", "dcl-one"] as const).map((s) => [
      s,
      { url: caps[s].url, error: caps[s].error, console: caps[s].console, blocked: [...new Set(caps[s].log.blocked)], unknown: [...new Set(caps[s].log.unknown)], height: caps[s].collected.height },
    ]),
  ) as PageResult["sides"];
  // The server-side render is cut off from the network like the browser: a host it tried counts as unknown.
  for (const host of new Set(ssrBlocked)) sides["dcl-one"].unknown.push(`SSR ${host}`);
  const result: PageResult = {
    page: spec.id,
    title: spec.title,
    viewport: vp.name,
    path: pagePath(spec, opts.targets),
    identical: false,
    pixels,
    structure,
    lines: structure.map(line),
    masks,
    regions,
    sides,
    misses: recording.misses.slice(missesBefore).map((m) => classifyMiss(m, spec)),
    unknown: (["reference", "dcl-one"] as const).flatMap((s) => sides[s].unknown.map((u) => classifyUnknown(s, u, spec))),
  };
  result.identical = isIdentical(result);
  writeFileSync(join(dir, "result.json"), `${JSON.stringify(result, null, 1)}\n`);
  return result;
}

export type RunOptions = {
  now: number;
  targets: Record<string, string>;
  out: string;
  onResult?: (r: PageResult) => void;
  beforePage?: (spec: PageSpec, browser: Browser, servers: Servers) => Promise<void>;
  onPage?: (side: Side, page: Page) => Promise<void>;
};

export async function runAll(recording: Recording, specs: PageSpec[], viewports: Viewport[], opts: RunOptions): Promise<{ results: PageResult[]; executable: string }> {
  const servers = await startServers(recording, opts.now);
  let browser: Browser | undefined;
  let executable = "";
  const results: PageResult[] = [];
  try {
    ({ browser, executable } = await launch());
    for (const spec of specs) {
      if (opts.beforePage) await opts.beforePage(spec, browser, servers);
      for (const vp of viewports) {
        const r = await comparePage(browser, recording, servers, spec, vp, opts);
        results.push(r);
        opts.onResult?.(r);
      }
    }
  } finally {
    try {
      await browser?.close();
    } finally {
      await servers.close();
    }
  }
  return { results, executable };
}

export function selectViewports(arg?: string): Viewport[] {
  if (!arg) return VIEWPORTS;
  const names = arg.split(",");
  return VIEWPORTS.filter((v) => names.includes(v.name));
}
