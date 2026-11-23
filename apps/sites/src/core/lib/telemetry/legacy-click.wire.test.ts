// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { LegacyClick, LegacySectionViewed, LegacySession } from "./legacy-click";

// The serialized Segment bodies a legacy Click or Section Viewed leaves the browser with, through the real
// transport and gates. Each test loads a fresh document, whose readiness is its first page view.

const BROWSER_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

const sendBeacon = vi.fn((_url: string, _body: Blob) => true);

function browser(userAgent = BROWSER_UA) {
  vi.stubGlobal("navigator", { userAgent, language: "en-US", maxTouchPoints: 0, sendBeacon });
}

async function loadDocument(path = "/") {
  window.history.replaceState(null, "", path);
  vi.stubGlobal("performance", { now: () => Date.now(), getEntriesByType: () => [] });
  vi.resetModules();
  const telemetry = await import("./track");
  const legacy = await import("./legacy-click");
  // A scope standing for the mounted control, as a component's `useLegacyTrackScope` would give it.
  const scope = legacy.createLegacyTrackScope();
  type Session = Omit<LegacySession, "scope">;
  return {
    page: telemetry.page,
    scope,
    trackLegacyClick: (click: LegacyClick, session: Session) => legacy.trackLegacyClick(click, { ...session, scope }),
    trackSectionViewed: (section: LegacySectionViewed, session: Session) =>
      legacy.trackSectionViewed(section, { ...session, scope }),
  };
}

async function tracks(): Promise<Array<{ event: string; userId?: string; anonymousId: string; properties: Record<string, unknown> }>> {
  const bodies = await Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await blob.text())));
  return bodies.filter((b) => b.type === "track");
}

beforeEach(() => {
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
  browser();
});
afterEach(() => {
  vi.unstubAllGlobals();
  sendBeacon.mockClear();
  window.history.replaceState(null, "", "/");
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
});

describe("a legacy Click", () => {
  it("leaves after the document's page view as a track named Click, for the visitor's session and wallet", async () => {
    const { page, trackLegacyClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackLegacyClick({ place: "Landing Explore", section: "view_all" }, { sid: "sid-1", userId: "0xAbC" });
    const [click] = await tracks();
    expect(click).toMatchObject({ event: "Click", anonymousId: "sid-1", userId: "0xabc" });
    expect(click!.properties).toMatchObject({ place: "Landing Explore", section: "view_all", track_deferred: false });
  });

  it("is dropped when its component unmounts before the document's page view, and never sent later", async () => {
    const { page, scope, trackLegacyClick } = await loadDocument("/create");
    trackLegacyClick({ place: "Creators Faqs", title: "faqs-cta" }, { sid: "sid-1", userId: "0xabc" });
    scope.close();
    window.history.replaceState(null, "", "/blog");
    page("/blog", {}, { sid: "sid-1" });
    expect((await tracks()).filter((t) => t.event === "Click")).toEqual([]);
  });

  it("is sent once, deferred, when its component is still mounted at the page view", async () => {
    const { page, trackLegacyClick } = await loadDocument("/create");
    trackLegacyClick({ place: "Creators Hero" }, { sid: "sid-1" });
    page("/create", {}, { sid: "sid-1" });
    page("/create", {}, { sid: "sid-1" });
    const clicks = (await tracks()).filter((t) => t.event === "Click");
    expect(clicks).toHaveLength(1);
    expect(clicks[0]!.properties).toMatchObject({ place: "Creators Hero", track_deferred: true });
  });

  it("is never sent for a bot", async () => {
    browser(BOT_UA);
    const { page, trackLegacyClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackLegacyClick({ place: "Landing Catch The Vibe" }, { sid: "sid-1" });
    expect(await tracks()).toEqual([]);
  });

  it("is never sent in a session that started on an exempt page, even after navigating to /create", async () => {
    const { page, trackLegacyClick } = await loadDocument("/download");
    page("/download", {}, { sid: "sid-1" });
    window.history.replaceState(null, "", "/create");
    trackLegacyClick({ place: "Creators Hero" }, { sid: "sid-1" });
    expect(await tracks()).toEqual([]);
  });

  it("is not sent without a write key", async () => {
    delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
    const { page, trackLegacyClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackLegacyClick({ place: "Landing Catch The Vibe" }, { sid: "sid-1" });
    expect(sendBeacon).not.toHaveBeenCalled();
  });
});

describe("Section Viewed", () => {
  it("leaves after the document's page view under its legacy name", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: false, media: query }));
    const { page, trackSectionViewed } = await loadDocument("/create");
    trackSectionViewed("Creators Why", { sid: "sid-1" });
    expect(await tracks()).toEqual([]);
    page("/create", {}, { sid: "sid-1" });
    const [viewed] = await tracks();
    expect(viewed).toMatchObject({ event: "Section Viewed", anonymousId: "sid-1" });
    expect(viewed!.properties).toMatchObject({ section_viewed: "Creators Why", mobile: false, track_deferred: true });
  });

  it("is dropped when its section unmounts before the document's page view", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: false, media: query }));
    const { page, scope, trackSectionViewed } = await loadDocument("/create");
    trackSectionViewed("Creators Why", { sid: "sid-1" });
    scope.close();
    page("/create", {}, { sid: "sid-1" });
    expect(await tracks()).toEqual([]);
  });

  it("is never sent for a bot", async () => {
    browser(BOT_UA);
    const { page, trackSectionViewed } = await loadDocument("/create");
    page("/create", {}, { sid: "sid-1" });
    trackSectionViewed("Creators Faqs", { sid: "sid-1" });
    expect(await tracks()).toEqual([]);
  });
});
