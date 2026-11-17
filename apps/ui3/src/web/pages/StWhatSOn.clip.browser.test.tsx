// @vitest-environment node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { findBrowser, openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs";
import StWhatSOn, { type WoDayEvent, type WoLiveCard, type WoUpcomingCard } from "./StWhatSOn";

const root = new URL("../../../../../", import.meta.url);
const css = [
  "library/ui/foundations/colors/colors.css",
  "library/ui/foundations/spacing/spacing.css",
  "library/ui/foundations/layout/layout.css",
  "apps/ui3/src/web/pages/stwhatson.css",
]
  .map((file) => readFileSync(fileURLToPath(new URL(file, root)), "utf8"))
  .join("\n");

const liveNow: WoLiveCard[] = [
  { id: "l1", title: "Live one", users: 3, isEvent: true, creator: "Ada", hue: 10 },
  { id: "l2", title: "Live two", users: 5, isEvent: false, creator: "Bo", hue: 90 },
];
const upcoming: WoUpcomingCard[] = Array.from({ length: 4 }, (_, i) => ({ id: `u${i}`, name: `Upcoming ${i}`, creator: "Cy", time: "Today 6:00 PM", hue: i * 40 }));
const day = (live: boolean): WoDayEvent[] => [
  { id: "d1", name: "Future", creator: "Di", time: "6:00 PM", live: false, hue: 120 },
  { id: "d2", name: "Now", creator: "Ed", time: "Now", live, users: 4, hue: 200 },
];
const allDays = [day(true), day(false), day(false), day(false), day(false)];

let session: Awaited<ReturnType<typeof openSession>>;
let startup: ReturnType<typeof openSession> | undefined;
const startupController = new AbortController();

async function evaluate<T>(expression: string): Promise<T> {
  const response = (await session.send("Runtime.evaluate", { expression, returnByValue: true })) as { result: { value: T } };
  return response.result.value;
}

async function load(width: number) {
  await session.send("Emulation.setDeviceMetricsOverride", { width, height: 1000, deviceScaleFactor: 1, mobile: false });
  const markup = renderToStaticMarkup(<StWhatSOn chrome={false} liveNow={liveNow} upcoming={upcoming} allDays={allDays} dayLabels={["Today", "Tue", "Wed", "Thu", "Fri"]} />);
  const html = `<!doctype html><style>body{margin:0}${css}*,*::before,*::after{transition:none!important}</style>${markup}`;
  await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
}

/** The first visible card matching the selector, scrolled into view. */
async function cardCentre(selector: string) {
  return evaluate<{ x: number; y: number } | null>(`(() => {
    const card = [...document.querySelectorAll(${JSON.stringify(selector)})].find((el) => el.getBoundingClientRect().width > 0);
    if (!card) return null;
    card.scrollIntoView({ block: "center", inline: "nearest" });
    document.querySelectorAll("[data-probe]").forEach((el) => el.removeAttribute("data-probe"));
    card.setAttribute("data-probe", "");
    const r = card.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + 8 };
  })()`);
}

/**
 * How far the probed card's painted box (its rectangle, transform included, grown by its box-shadow's offset, blur and
 * spread) reaches past the padding box of each ancestor that clips. Empty when nothing clips it.
 */
const clipped = `(() => {
  const card = document.querySelector("[data-probe]");
  const shadow = getComputedStyle(card).boxShadow;
  const n = shadow === "none" ? [0, 0, 0, 0] : (shadow.match(/-?[\\d.]+px/g) || []).map(parseFloat);
  const [x = 0, y = 0, blur = 0, spread = 0] = n;
  const r = card.getBoundingClientRect();
  const paint = { left: r.left + x - blur - spread, right: r.right + x + blur + spread, top: r.top + y - blur - spread, bottom: r.bottom + y + blur + spread };
  const out = [];
  for (let el = card.parentElement; el && el !== document.body; el = el.parentElement) {
    const s = getComputedStyle(el);
    const a = el.getBoundingClientRect();
    const box = { left: a.left + parseFloat(s.borderLeftWidth), right: a.right - parseFloat(s.borderRightWidth), top: a.top + parseFloat(s.borderTopWidth), bottom: a.bottom - parseFloat(s.borderBottomWidth) };
    if (s.overflowX !== "visible" && (paint.left < box.left - 0.5 || paint.right > box.right + 0.5)) out.push(el.className + " x");
    if (s.overflowY !== "visible" && (paint.top < box.top - 0.5 || paint.bottom > box.bottom + 0.5)) out.push(el.className + " y");
  }
  return { shadow, transform: getComputedStyle(card).transform, out };
})()`;

type Clip = { shadow: string; transform: string; out: string[] };

beforeAll(async () => {
  startup = (async () =>
    openSession(
      await findBrowser([
        process.env.BROWSER_PATH,
        process.env.CHROME_PATH,
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
      ]),
      { signal: startupController.signal },
    ))();
  session = await startup;
}, 40_000);

afterAll(async () => {
  startupController.abort(new Error("Clip fixture disposed"));
  const opened = await startup?.catch(() => undefined);
  await opened?.close();
}, 30_000);

const cards = [
  ["an Upcoming card", ".wo-up .wo-sm"],
  ["an All Events card", ".wo-all .wo-fc, .wo-all__list .wo-sm"],
  ["a Live Now card", ".wo-ln__card"],
] as const;

describe.each([390, 768, 1440])("when What's On renders at %ipx", (width) => {
  it.each(cards)("should paint %s's hover state inside every clipping ancestor", async (_, selector) => {
    await load(width);
    const point = await cardCentre(selector);
    expect(point).not.toBeNull();
    await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point!.x, y: point!.y });
    expect((await evaluate<Clip>(clipped)).out).toEqual([]);
  });

  it.each(cards)("should paint %s's keyboard focus state inside every clipping ancestor", async (_, selector) => {
    await load(width);
    expect(await cardCentre(selector)).not.toBeNull();
    await evaluate(`document.querySelector("[data-probe] a").focus()`);
    expect((await evaluate<Clip>(clipped)).out).toEqual([]);
  });

  it("should neither lift nor glow the Upcoming and All Events cards, as in Figma", async () => {
    await load(width);
    for (const selector of [".wo-up .wo-sm", ".wo-all .wo-fc, .wo-all__list .wo-sm"]) {
      const point = await cardCentre(selector);
      await session.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point!.x, y: point!.y });
      expect(await evaluate<Clip>(clipped)).toMatchObject({ shadow: "none", transform: "none" });
    }
  });
});
