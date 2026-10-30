// Capture one page on one side: settle, run the page's interactions, read the structure, and take
// the page as a user scrolls it (viewport tiles stitched by their real scroll offset, so fixed
// backgrounds and sticky bars are painted as they are seen, not stretched by a full-page grab).
import type { Browser, Page } from "playwright-core";
import { pagePath, type PageSpec, type Side } from "../pages.mts";
import { sideContext, type Viewport } from "./browser.mts";
import { collectInPage, type Box, type Collected } from "./collect.mts";
import { decodePng, encodePng, type Rgba } from "./png.mts";
import type { Recording } from "./recording.mts";
import { routeSide, type SideLog } from "./route.mts";

export type SideCapture = {
  side: Side;
  url: string;
  png: Buffer;
  collected: Collected;
  masks: Box[];
  regions: Box[];
  /** The regions' boxes per declared region, in spec order. */
  regionGroups: Box[][];
  console: string[];
  log: SideLog;
  error?: string;
};

const BUSY = "[aria-busy=true], [data-testid*=skeleton], .skeleton, [class*=Skeleton], [class*=skeleton]";

async function settle(page: Page, ready?: string): Promise<void> {
  await page.waitForLoadState("networkidle", { timeout: 15_000 }).catch(() => {});
  if (ready) await page.locator(ready).first().waitFor({ state: "visible", timeout: 20_000 });
  await page
    .waitForFunction((busy: string) => ![...document.querySelectorAll(busy)].some((e) => e.getBoundingClientRect().height > 0), BUSY, { timeout: 15_000 })
    .catch(() => {});
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all(
      [...document.images].filter((i) => !i.complete).map((i) => new Promise((r) => { i.addEventListener("load", r, { once: true }); i.addEventListener("error", r, { once: true }); setTimeout(r, 5000); })),
    );
  });
  await page.waitForLoadState("networkidle", { timeout: 10_000 }).catch(() => {});
  await page.waitForTimeout(300);
}

const docHeight = (page: Page, cap?: number) => page.evaluate((c: number) => Math.min(document.documentElement.scrollHeight, c), cap ?? Number.MAX_SAFE_INTEGER);

/** Scroll through once (to the page's depth cap) so lazy rails and images load, then return to the top. */
async function warm(page: Page, vh: number, cap?: number): Promise<void> {
  const height = await docHeight(page, cap);
  for (let y = 0; y < height; y += Math.round(vh * 0.8)) {
    await page.evaluate((top: number) => window.scrollTo(0, top), y);
    await page.waitForTimeout(120);
  }
  await page.evaluate((top: number) => window.scrollTo(0, top), Math.max(0, height - vh));
  await page.waitForTimeout(300);
  await page.evaluate(() => window.scrollTo(0, 0));
}

async function frames(page: Page) {
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(null)))));
}

/**
 * Sticky elements are measured and painted once, where the layout puts them: relative with no offset
 * from measuring on, so a panel below the first screen is compared like any other content and its
 * boxes match its pixels. Both sides get the same treatment.
 */
async function unstick(page: Page): Promise<void> {
  await page.evaluate(() => {
    for (const el of document.body.querySelectorAll("*")) {
      if (getComputedStyle(el).position !== "sticky") continue;
      const style = (el as HTMLElement).style;
      style.setProperty("position", "relative", "important");
      for (const side of ["top", "right", "bottom", "left"]) style.setProperty(side, "auto", "important");
    }
  });
}

/**
 * Fixed bars (under half the viewport tall) are painted once, in the first tile; later tiles hide them
 * so the stitched page is not striped with copies. Full-height fixed layers (the page field) stay.
 * Both sides get the same treatment.
 */
async function hideBars(page: Page, vh: number, hide: boolean): Promise<void> {
  await page.evaluate(
    ([limit, on]) => {
      const KEY = "data-parity-hidden";
      if (!on) {
        for (const el of document.querySelectorAll(`[${KEY}]`)) {
          (el as HTMLElement).style.visibility = el.getAttribute(KEY)!;
          el.removeAttribute(KEY);
        }
        return;
      }
      for (const el of document.body.querySelectorAll("*")) {
        const cs = getComputedStyle(el);
        if (cs.position !== "fixed") continue;
        const r = el.getBoundingClientRect();
        if (r.height === 0 || r.height >= (limit as number) / 2) continue;
        el.setAttribute(KEY, (el as HTMLElement).style.visibility);
        (el as HTMLElement).style.visibility = "hidden";
      }
    },
    [vh, hide] as const,
  );
}

async function tiles(page: Page, vp: Viewport, cap?: number): Promise<Rgba> {
  const height = await docHeight(page, cap);
  const out = new Uint8Array(vp.width * height * 4);
  let filled = 0;
  for (let y = 0; filled < height; y += vp.height) {
    await page.evaluate((top: number) => window.scrollTo(0, top), y);
    await frames(page);
    await page.waitForTimeout(150);
    const real = await page.evaluate(() => Math.round(window.scrollY));
    const shot = decodePng(await page.screenshot({ animations: "disabled", caret: "hide", scale: "css" }));
    for (let row = Math.max(0, filled - real); row < shot.height && real + row < height; row++) {
      out.set(shot.data.subarray(row * shot.width * 4, row * shot.width * 4 + vp.width * 4), (real + row) * vp.width * 4);
    }
    const next = Math.min(height, real + shot.height);
    if (next <= filled) break;
    if (filled === 0) await hideBars(page, vp.height, true);
    filled = next;
  }
  await hideBars(page, vp.height, false);
  await page.evaluate(() => window.scrollTo(0, 0));
  return { width: vp.width, height: filled, data: out.subarray(0, vp.width * filled * 4) };
}

export async function captureSide(
  browser: Browser,
  spec: PageSpec,
  vp: Viewport,
  side: Side,
  opts: { origin: string; recording: Recording; now: number; seed: number; targets: Record<string, string>; onPage?: (page: Page) => Promise<void> },
): Promise<SideCapture> {
  const context = await sideContext(browser, vp, { now: opts.now, seed: opts.seed });
  const log = await routeSide(context, opts.recording, side, opts.origin);
  const page = await context.newPage();
  const consoleLines: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning") consoleLines.push(`${m.type()}: ${m.text().slice(0, 300)}`);
  });
  page.on("pageerror", (e) => consoleLines.push(`pageerror: ${String(e).slice(0, 300)}`));
  const keys = spec.keys.map((k) => ({ name: k.name, selector: typeof k.selector === "string" ? k.selector : k.selector[side], max: k.max ?? 1 }));
  const masks = (spec.masks ?? []).flatMap((m) => m.selector[side]);
  const regions = (spec.regions ?? []).flatMap((r) => r.selector[side]);
  try {
    await page.goto(new URL(pagePath(spec, opts.targets), opts.origin).href, { waitUntil: "load" });
    if (spec.setup) {
      await settle(page);
      await spec.setup(page, side, opts.targets);
    }
    await settle(page, spec.ready[side]);
    await warm(page, vp.height, spec.maxHeight);
    // A transition started by the scroll back to the top (a bar deepening on scroll) would be measured at
    // whatever point each app's handler happened to start it; end states are what is compared.
    await page.addStyleTag({ content: "*, *::before, *::after { transition-duration: 0s !important; transition-delay: 0s !important; }" });
    await settle(page);
    if (opts.onPage) await opts.onPage(page);
    await unstick(page);
    await frames(page);
    const collected = await page.evaluate(collectInPage, { keys, masks: [...masks, ...regions] });
    const boxes = await page.evaluate(collectInPage, { keys: [], masks });
    const regionBoxes = await page.evaluate(collectInPage, { keys: [], masks: regions });
    const regionGroups: Box[][] = [];
    for (const r of spec.regions ?? []) regionGroups.push((await page.evaluate(collectInPage, { keys: [], masks: r.selector[side] })).masks);
    const png = encodePng(await tiles(page, vp, spec.maxHeight));
    if (spec.maxHeight) {
      const below = (b: { y: number }) => b.y < spec.maxHeight!;
      collected.nodes = collected.nodes.filter((n) => below(n.box));
      collected.texts = collected.texts.filter((n) => below(n.box));
      collected.height = Math.min(collected.height, spec.maxHeight);
    }
    return { side, url: page.url(), png, collected, masks: boxes.masks, regions: regionBoxes.masks, regionGroups, console: consoleLines, log };
  } catch (err) {
    const png = await page.screenshot({ fullPage: true }).catch(() => Buffer.alloc(0));
    return {
      side,
      url: page.url(),
      png,
      collected: { url: page.url(), width: vp.width, height: 0, nodes: [], texts: [], masks: [] },
      masks: [],
      regions: [],
      regionGroups: [],
      console: consoleLines,
      log,
      error: String(err).split("\n")[0],
    };
  } finally {
    await context.close();
  }
}
