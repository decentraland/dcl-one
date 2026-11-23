// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";

// The funnel's events through the real transport: the legacy beacon sent them on an exempt session and for a bot.

const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
const sendBeacon = vi.fn((_url: string, _body: Blob) => true);

async function loadDocument(path: string, userAgent: string) {
  window.history.replaceState(null, "", path);
  vi.stubGlobal("navigator", { userAgent, language: "en-US", maxTouchPoints: 0, sendBeacon });
  vi.spyOn(performance, "getEntriesByType").mockReturnValue([]);
  vi.resetModules();
  return import("./download-funnel");
}

async function events(): Promise<string[]> {
  const bodies = await Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await blob.text())));
  return bodies.map((b) => b.event);
}

beforeEach(() => {
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  sendBeacon.mockClear();
  window.history.replaceState(null, "", "/");
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
});

it("sends a bot's funnel on a session that started on the exempt /download", async () => {
  const funnel = await loadDocument("/download", BOT_UA);
  funnel.sendDownloadPageExit(10, { sid: "sid-1" });
  funnel.sendDownloadSuccessArrived({ os: "macOS", arch: "arm64", place: "download-page", revisit: 0, authenticated: false }, { sid: "sid-1" });
  expect(await events()).toEqual(["download_page_exit", "download_success_arrived"]);
});

it("sends nothing without a write key", async () => {
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
  const funnel = await loadDocument("/download_success", BOT_UA);
  funnel.sendDownloadPageExit(10, { sid: "sid-1" });
  expect(sendBeacon).not.toHaveBeenCalled();
});
