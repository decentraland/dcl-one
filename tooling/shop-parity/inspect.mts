// Evaluate an expression in one side's page, over the recording, to find selectors and causes.
//
//   node inspect.mts <page-id|/path> <desktop|mobile> <reference|dcl-one> '<js expression>'
//   node inspect.mts item desktop dcl-one 'document.querySelector("[data-testid=item-price]").outerHTML'
//
// A page id runs that page's interactions first (the cart adds its item).
import { join } from "node:path";
import { PAGES, pagePath } from "./pages.mts";
import { defaultSet } from "./capture.mts";
import { launch, sideContext, VIEWPORTS } from "./lib/browser.mts";
import { RECORDINGS } from "./lib/paths.mts";
import { Recording } from "./lib/recording.mts";
import { routeSide } from "./lib/route.mts";
import { SEED, startServers } from "./lib/run.mts";

const [target, vpName, side, expr] = process.argv.slice(2);
if (!target || !vpName || (side !== "reference" && side !== "dcl-one") || !expr) {
  console.error("usage: node inspect.mts <page-id|/path> <desktop|mobile> <reference|dcl-one> '<js expression>'");
  process.exit(2);
}
const recording = new Recording(join(RECORDINGS, process.env.SHOP_PARITY_SET ?? defaultSet()));
await recording.fetchAssets();
const spec = PAGES.find((p) => p.id === target);
const targets = recording.manifest?.targets ?? {};
const now = Date.parse(recording.manifest!.recordedAt);
const servers = await startServers(recording, now);
let browser: Awaited<ReturnType<typeof launch>>["browser"] | undefined;
try {
  ({ browser } = await launch());
  const vp = VIEWPORTS.find((v) => v.name === vpName) ?? VIEWPORTS[0]!;
  const context = await sideContext(browser, vp, { now, seed: SEED });
  await routeSide(context, recording, side, servers.origins[side]);
  const page = await context.newPage();
  await page.goto(new URL(spec ? pagePath(spec, targets) : target, servers.origins[side]).href, { waitUntil: "load" });
  if (spec?.setup) await spec.setup(page, side, targets);
  await page.waitForLoadState("networkidle", { timeout: 15_000 }).catch(() => {});
  await page.waitForTimeout(1000);
  console.log(JSON.stringify(await page.evaluate(expr), null, 1));
} finally {
  await browser?.close();
  await servers.close();
}
