// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { dropTrackedUser, restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";

import { createLegacyTrackScope, type LegacySession } from "@/core/lib/telemetry/legacy-click";

import { trackDownloadClick as trackInThisDocument, type DownloadClick, type DownloadClickOptions } from "./download-click";

type Session = Omit<LegacySession, "scope">;

/** A scope standing for the mounted control, as a component's `useLegacyTrackScope` would give it. */
const trackDownloadClick = (click: DownloadClick, session: Session, options?: DownloadClickOptions) =>
  trackInThisDocument(click, { ...session, scope: createLegacyTrackScope() }, options);

// The serialized Segment body a download Click leaves the browser with, through the real transport.

const BROWSER_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const UA_DATA = { brands: [{ brand: "Chromium", version: "131" }], mobile: false, platform: "Windows", architecture: "x86" };

const sendBeacon = vi.fn((_url: string, _body: Blob) => true);

function browser(userAgentData?: unknown) {
  vi.stubGlobal("navigator", {
    userAgent: BROWSER_UA,
    language: "en-US",
    maxTouchPoints: 0,
    sendBeacon,
    ...(userAgentData ? { userAgentData } : {}),
  });
}

async function body(): Promise<Record<string, unknown> & { context: Record<string, unknown> }> {
  expect(sendBeacon).toHaveBeenCalledTimes(1);
  return JSON.parse(await sendBeacon.mock.calls[0]![1].text());
}

beforeEach(() => {
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
  browser();
});
afterEach(() => {
  vi.unstubAllGlobals();
  sendBeacon.mockClear();
  restoreTrackedUser();
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
});

const CLICK = { place: "Landing Hero", event: "Download", download_target: "desktop_installer" } as const;

it("names a signed-in visitor by the verified wallet, lowercased", async () => {
  trackDownloadClick(CLICK, { sid: "sid-1", userId: "0xAbC" });
  const sent = await body();
  expect(sent).toMatchObject({ type: "track", event: "Click", anonymousId: "sid-1", userId: "0xabc" });
  expect(sent.properties).toMatchObject({ place: "Landing Hero", event: "Download", download_target: "desktop_installer" });
});

it("sends no userId for an anonymous visitor", async () => {
  trackDownloadClick(CLICK, { sid: "sid-1" });
  expect(await body()).not.toHaveProperty("userId");
});

it("sends no userId once a sign-out has started", async () => {
  dropTrackedUser();
  trackDownloadClick(CLICK, { sid: "sid-1", userId: "0xabc" });
  expect(await body()).not.toHaveProperty("userId");
});

it("carries the low-entropy userAgentData when the browser exposes it", async () => {
  browser(UA_DATA);
  trackDownloadClick(CLICK, { sid: "sid-1" });
  expect((await body()).context.userAgentData).toEqual({
    brands: [{ brand: "Chromium", version: "131" }],
    mobile: false,
    platform: "Windows",
  });
});

it("leaves userAgentData out where the browser has none", async () => {
  trackDownloadClick(CLICK, { sid: "sid-1" });
  expect((await body()).context).not.toHaveProperty("userAgentData");
});

// track_deferred through the real gates: each test loads a fresh document, whose readiness is its first page view.

const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

async function loadDocument(path = "/") {
  window.history.replaceState(null, "", path);
  vi.stubGlobal("performance", { ...performance, getEntriesByType: () => [] });
  vi.resetModules();
  const telemetry = await import("@/core/lib/telemetry/track");
  const download = await import("./download-click");
  const legacy = await import("@/core/lib/telemetry/legacy-click");
  const scope = legacy.createLegacyTrackScope();
  return {
    page: telemetry.page,
    scope,
    trackDownloadClick: (click: DownloadClick, session: Session, options?: DownloadClickOptions) =>
      download.trackDownloadClick(click, { ...session, scope }, options),
  };
}

async function clicks(): Promise<Array<Record<string, unknown>>> {
  const bodies = await Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await blob.text())));
  return bodies.filter((b) => b.type === "track" && b.event === "Click").map((b) => b.properties);
}

const TRACK_CLICK = { conversion: false, correlate: false } as const;

describe("track_deferred", () => {
  afterEach(() => {
    window.history.replaceState(null, "", "/");
  });

  it("is false for a click after the document's page view", async () => {
    const { page, trackDownloadClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackDownloadClick(CLICK, { sid: "sid-1" });
    expect(await clicks()).toEqual([expect.objectContaining({ track_deferred: false })]);
  });

  it("is true for a conversion clicked before the document's first page view, sent at once", async () => {
    const { trackDownloadClick } = await loadDocument("/");
    trackDownloadClick(CLICK, { sid: "sid-1" });
    expect(sendBeacon).toHaveBeenCalledOnce();
    expect(await clicks()).toEqual([expect.objectContaining({ track_deferred: true })]);
  });

  it("holds a non-conversion click until the first page view, then sends it deferred", async () => {
    const { page, trackDownloadClick } = await loadDocument("/download/creator-hub");
    const now = vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackDownloadClick({ ...CLICK, place: "Download", download_target: "creator_hub" }, { sid: "sid-1" }, TRACK_CLICK);
    expect(await clicks()).toEqual([]);

    now.mockReturnValue(1_700_000_000_400);
    page("/download/creator-hub", {}, { sid: "sid-1" });
    expect(await clicks()).toEqual([
      expect.objectContaining({ track_called_at: 1_700_000_000_000, track_delivered_at: 1_700_000_000_400, track_deferred: true }),
    ]);
    now.mockRestore();
  });

  it("drops a non-conversion click whose control unmounted before the first page view", async () => {
    const { page, scope, trackDownloadClick } = await loadDocument("/download/creator-hub");
    trackDownloadClick({ ...CLICK, place: "Download", download_target: "creator_hub" }, { sid: "sid-1", userId: "0xabc" }, TRACK_CLICK);
    scope.close();
    window.history.replaceState(null, "", "/");
    page("/", {}, { sid: "sid-1" });
    expect(await clicks()).toEqual([]);
  });

  it("still sends a conversion whose control unmounted at once", async () => {
    const { scope, trackDownloadClick } = await loadDocument("/");
    trackDownloadClick(CLICK, { sid: "sid-1" });
    scope.close();
    expect(await clicks()).toEqual([expect.objectContaining({ place: "Landing Hero", track_deferred: true })]);
  });

  it("is true for every conversion in a session that started on /download, whose page view is held back", async () => {
    const { page, trackDownloadClick } = await loadDocument("/download");
    page("/download", {}, { sid: "sid-1" });
    trackDownloadClick({ ...CLICK, place: "Download" }, { sid: "sid-1" });
    expect(await clicks()).toEqual([expect.objectContaining({ track_deferred: true })]);
  });

  it("never sends a non-conversion click in a session that started on /download", async () => {
    const { page, trackDownloadClick } = await loadDocument("/download");
    page("/download", {}, { sid: "sid-1" });
    trackDownloadClick({ ...CLICK, place: "Download", download_target: "creator_hub" }, { sid: "sid-1" }, TRACK_CLICK);
    expect(await clicks()).toEqual([]);
  });

  it("is true for a bot's conversion, and a bot's non-conversion click is never sent", async () => {
    vi.stubGlobal("navigator", { userAgent: BOT_UA, language: "en-US", maxTouchPoints: 0, sendBeacon });
    const { page, trackDownloadClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackDownloadClick(CLICK, { sid: "sid-1" });
    trackDownloadClick({ ...CLICK, place: "Download", download_target: "creator_hub" }, { sid: "sid-1" }, TRACK_CLICK);
    expect(await clicks()).toEqual([expect.objectContaining({ place: "Landing Hero", track_deferred: true })]);
  });

  it("stays false after a client navigation to /download from a tracked page", async () => {
    const { page, trackDownloadClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    window.history.replaceState(null, "", "/download");
    trackDownloadClick({ ...CLICK, place: "Download" }, { sid: "sid-1" });
    expect(await clicks()).toEqual([expect.objectContaining({ track_deferred: false })]);
  });

  it("stays true when the page view could not be sent for want of a write key", async () => {
    delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
    const { page, trackDownloadClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
    trackDownloadClick(CLICK, { sid: "sid-1" });
    expect(await clicks()).toEqual([expect.objectContaining({ track_deferred: true })]);
  });
});
