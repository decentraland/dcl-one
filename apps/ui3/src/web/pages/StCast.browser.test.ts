import { readFileSync } from "node:fs";
import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { findBrowser, openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs";
import { StCastStreamer, StCastWatcher } from "@dcl/one-ui";

const css = ["../../atoms/primitives.css", "../../../../../library/ui/apps/sites/cast/stcaststreamer.css", "../../../../../library/ui/apps/sites/cast/stcastwatcher.css"]
  .map((file) => readFileSync(new URL(file, import.meta.url), "utf8"))
  .join("\n");

const WORLD = `0x${"ab".repeat(20)}.dcl.eth`;
const stage = createElement("video");
const notice = { id: "audio", title: "Audio is paused", message: "Your browser needs a click before the Cast can play sound." };
const pages = {
  streamerLobby: createElement(StCastStreamer, { state: "onboarding", streamName: WORLD }),
  streamerLive: createElement(StCastStreamer, { state: "live", stage, participants: 2 }),
  streamerNotice: createElement(StCastStreamer, { state: "live", stage, toasts: [notice] }),
  watcherLobby: createElement(StCastWatcher, { state: "onboarding", streamName: WORLD }),
  watcherLive: createElement(StCastWatcher, { state: "live", stage, sidebarOpen: false }),
  watcherChat: createElement(StCastWatcher, { state: "live", stage, sidebarOpen: true }),
  watcherRetry: createElement(StCastWatcher, { state: "joining", joiningLabel: "Reconnecting\u{2026} (1 of 3)", canLeave: true }),
  watcherEnded: createElement(StCastWatcher, { state: "ended", watcherTitle: "You were removed from this Cast" }),
  watcherNotice: createElement(StCastWatcher, { state: "live", stage, sidebarOpen: false, toasts: [notice] }),
};

type Layout = { overlaps: string[]; outside: string[]; cut: string[]; overflow: number };

/**
 * The site header, the page's controls, titles, notices and LIVE pill as drawn: which overlap, which leave the
 * viewport, which titles are cut, the sideways overflow.
 */
const LAYOUT = `(() => {
  const box = (el) => {
    const r = el.getBoundingClientRect();
    const cut = el.matches("h1, h2") && el.scrollWidth > el.clientWidth;
    return { el, name: el.getAttribute("aria-label") || el.textContent.trim(), cut, l: r.left, t: r.top, r: r.right, b: r.bottom };
  };
  const drawn = "header, button, input, [role=status], h1, h2, .stcastwatcher__livepill, .stcast__livepill";
  const boxes = [...document.querySelectorAll(":is(" + drawn + "):not([inert] *)")].map(box);
  const overlaps = [];
  for (const [i, a] of boxes.entries())
    for (const b of boxes.slice(i + 1))
      if (!a.el.contains(b.el) && a.l < b.r && b.l < a.r && a.t < b.b && b.t < a.b) overlaps.push(a.name + " / " + b.name);
  const outside = boxes.filter((c) => c.l < 0 || c.t < 0 || c.r > innerWidth || c.b > innerHeight).map((c) => c.name);
  const cut = boxes.filter((c) => c.cut).map((c) => c.name);
  return { overlaps, outside, cut, overflow: document.documentElement.scrollWidth - innerWidth };
})()`;

describe.each([
  { width: 390, height: 844 },
  { width: 768, height: 1024 },
  { width: 1280, height: 720 },
])("when a Cast page opens at $width × $height", ({ width, height }) => {
  let session: Awaited<ReturnType<typeof openSession>>;

  beforeAll(async () => {
    session = await openSession(
      await findBrowser([process.env.BROWSER_PATH, process.env.CHROME_PATH, "/usr/bin/google-chrome", "/usr/bin/chromium"]),
    );
    await session.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  }, 40_000);

  afterAll(async () => session?.close(), 30_000);

  async function evaluate<T>(page: ReactElement, expression: string): Promise<T> {
    // The site header above the page: 64px below 992px, 92px from there.
    const html = `<!doctype html><style>body{margin:0}${css}</style><div class="ui2">
      <header aria-label="Site header" style="height:${width < 992 ? 64 : 92}px"></header>${renderToStaticMarkup(page)}</div>`;
    await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
    const { result } = (await session.send("Runtime.evaluate", { expression, returnByValue: true })) as { result: { value: T } };
    return result.value;
  }

  it.each(Object.keys(pages) as Array<keyof typeof pages>)("should draw every control of %s apart and in view", async (page) => {
    expect(await evaluate<Layout>(pages[page], LAYOUT)).toEqual({ overlaps: [], outside: [], cut: [], overflow: 0 });
  });

  it("should centre the speaker row of the watcher lobby in its card, around all its content", async () => {
    const row = await evaluate<{ offset: number; spill: number }>(
      pages.watcherLobby,
      `(() => {
        const row = document.querySelector(".stcastwatcher__selectorrow");
        const mid = (el) => el.getBoundingClientRect().left + el.getBoundingClientRect().width / 2;
        return { offset: Math.round(mid(row) - mid(document.querySelector(".stcastwatcher__modal"))), spill: row.scrollWidth - row.clientWidth };
      })()`,
    );
    expect(row).toEqual({ offset: 0, spill: 0 });
  });

  it("should keep the video in view beside the open chat", async () => {
    const video = await evaluate<{ width: number; height: number; covered: boolean }>(
      pages.watcherChat,
      `(() => {
        const v = document.querySelector(".stcastwatcher__videoarea").getBoundingClientRect();
        const s = document.querySelector(".stcastwatcher__sidebar").getBoundingClientRect();
        return { width: v.width, height: v.height, covered: v.left < s.right && s.left < v.right && v.top < s.bottom && s.top < v.bottom };
      })()`,
    );
    expect(video.covered).toBe(false);
    expect(Math.min(video.width, video.height)).toBeGreaterThanOrEqual(200);
  });
});
