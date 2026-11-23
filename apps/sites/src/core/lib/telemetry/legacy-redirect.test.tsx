// @vitest-environment jsdom
import { cleanup, render, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

// What the page a legacy redirect lands on puts on the wire, through the real transport and gates.

const BROWSER_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
const COOKIE = "dcl_legacy_redirect";
const NOW = new Date("2026-11-23T12:00:00.000Z");

const EVENTS_REDIRECT = {
  origin: "events",
  source: "/whats-on?tab=live",
  destination: "/events?tab=live",
  preservedParams: { tab: "live" },
};

let sendBeacon: ReturnType<typeof vi.fn>;

function handOver(details: unknown) {
  document.cookie = `${COOKIE}=${encodeURIComponent(JSON.stringify(details))}; Path=/`;
}

function cookieLeft(): boolean {
  return document.cookie.split(";").some((part) => part.trim().startsWith(`${COOKIE}=`));
}

/**
 * Loads a fresh document on `documentPath` (its readiness and exempt-session decision are per document) and
 * renders the root's two hooks, as `root.tsx` does, on `routePath`.
 */
async function landOn(routePath: string, { documentPath = routePath, userId = "" } = {}) {
  window.history.replaceState(null, "", documentPath);
  vi.spyOn(performance, "getEntriesByType").mockReturnValue([]);
  vi.resetModules();
  const { useRoutePageView } = await import("./page-view");
  const { useLegacyRedirectEvent } = await import("./legacy-redirect");
  function Root() {
    useRoutePageView();
    useLegacyRedirectEvent();
    return null;
  }
  const router = createMemoryRouter(
    [{ id: "root", path: "*", loader: () => ({ sid: "sid-1", analyticsUserId: userId }), Component: Root, HydrateFallback: () => null }],
    { initialEntries: [routePath] },
  );
  render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
}

async function bodies(): Promise<Array<Record<string, unknown>>> {
  return Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await (blob as Blob).text())));
}

async function redirectEvents() {
  return (await bodies()).filter((b) => b.type === "track");
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
  Object.defineProperty(navigator, "userAgent", { value: BROWSER_UA, configurable: true });
  sendBeacon = vi.fn(() => true);
  Object.defineProperty(navigator, "sendBeacon", { value: sendBeacon, configurable: true });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  document.cookie = `${COOKIE}=; Path=/; Max-Age=0`;
  Reflect.deleteProperty(navigator, "userAgent");
  Reflect.deleteProperty(navigator, "sendBeacon");
  Reflect.deleteProperty(window, "__DCL_PUBLIC__");
  window.history.replaceState(null, "", "/");
});

describe("a page a legacy redirect landed on", () => {
  it("sends Legacy Events Redirected once, after its page view, with the legacy properties", async () => {
    handOver(EVENTS_REDIRECT);
    await landOn("/events?tab=live", { userId: "0xAbC" });
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(2));
    const [view, event] = await bodies();
    expect(view).toMatchObject({ type: "page", name: "/events" });
    expect(event).toMatchObject({ type: "track", event: "Legacy Events Redirected", anonymousId: "sid-1", userId: "0xabc" });
    expect(event!.properties).toEqual({
      source: "/whats-on?tab=live",
      destination: "/events?tab=live",
      origin: "events",
      preservedParams: { tab: "live" },
      timestamp: "2026-11-23T12:00:00.000Z",
    });
    expect(Object.keys(event!.properties as object)).toEqual(["source", "destination", "origin", "preservedParams", "timestamp"]);
    expect(cookieLeft()).toBe(false);
  });

  it("sends Legacy Places Redirected for a Discover URL, without a userId for an anonymous visitor", async () => {
    handOver({ origin: "places", source: "/discover", destination: "/places", preservedParams: {} });
    await landOn("/events");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(2));
    const [event] = await redirectEvents();
    expect(event).toMatchObject({ event: "Legacy Places Redirected", properties: { origin: "places", source: "/discover" } });
    expect(event).not.toHaveProperty("userId");
  });

  it("sends nothing for a bot, as the legacy SDK never loaded for one", async () => {
    Object.defineProperty(navigator, "userAgent", { value: BOT_UA, configurable: true });
    handOver(EVENTS_REDIRECT);
    await landOn("/events");
    await waitFor(() => expect(cookieLeft()).toBe(false));
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it("sends nothing in a session that started on an exempt page", async () => {
    handOver(EVENTS_REDIRECT);
    await landOn("/events", { documentPath: "/download" });
    await waitFor(() => expect(cookieLeft()).toBe(false));
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it("drops a malformed hand-over without sending it", async () => {
    handOver({ ...EVENTS_REDIRECT, origin: "blog" });
    await landOn("/events");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
    expect(cookieLeft()).toBe(false);
  });

  it("drops a hand-over whose params are not all strings", async () => {
    handOver({ ...EVENTS_REDIRECT, preservedParams: { tab: 1 } });
    await landOn("/events");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
  });

  it("sends only its page view when no redirect led to it", async () => {
    await landOn("/events");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
  });
});
