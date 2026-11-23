// @vitest-environment jsdom
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { createElement, StrictMode } from "react";
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
const PREFIX = "dcl_legacy_redirect_";
const NOW = new Date("2026-11-23T12:00:00.000Z");

const EVENTS_REDIRECT = {
  origin: "events",
  source: "/whats-on?tab=live",
  destination: "/events?tab=live",
  preservedParams: { tab: "live" },
};
const PLACES_REDIRECT = { origin: "places", source: "/discover", destination: "/places", preservedParams: {} };
const A = "a1a1a1a1a1a1a1a1";
const B = "b2b2b2b2b2b2b2b2";
/** The URL a redirect sent the visitor to: the destination plus its hand-over's fragment. */
const via = (destination: string, id: string) => `${destination}#dcl-legacy-redirect=${id}`;

let sendBeacon: ReturnType<typeof vi.fn>;
/** The root loader's account; a test changes it and revalidates to re-render the root with another one. */
let rootUser = "";

function handOver(id: string, details: unknown) {
  document.cookie = `${PREFIX}${id}=${encodeURIComponent(JSON.stringify(details))}; Path=/`;
}

function handOversLeft(): string[] {
  return document.cookie
    .split(";")
    .map((part) => part.trim())
    .filter((part) => part.startsWith(PREFIX))
    .map((part) => part.slice(PREFIX.length, part.indexOf("=")));
}

/**
 * Loads a fresh document on `documentPath` (its readiness and exempt-session decision are per document) and
 * renders the root's two hooks in `root.tsx`'s order on `routePath`.
 */
async function landOn(routePath: string, { documentPath = routePath, userId = "" } = {}) {
  window.history.replaceState(null, "", documentPath);
  rootUser = userId;
  // The document's navigation entry: the path this document was loaded on, whatever the URL becomes.
  const loaded = { name: new URL(documentPath, window.location.origin).href } as PerformanceEntry;
  vi.spyOn(performance, "getEntriesByType").mockReturnValue([loaded]);
  vi.resetModules();
  const transport = await import("./track");
  const { useRoutePageView } = await import("./page-view");
  const { useLegacyRedirectEvent } = await import("./legacy-redirect");
  function Root() {
    useLegacyRedirectEvent();
    useRoutePageView();
    return null;
  }
  const router = createMemoryRouter(
    [{ id: "root", path: "*", loader: () => ({ sid: "sid-1", analyticsUserId: rootUser }), Component: Root, HydrateFallback: () => null }],
    { initialEntries: [routePath] },
  );
  const view = render(createElement(StrictMode, null, createElement(RouterProvider, { router })));
  return { page: transport.page, router, view };
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
  for (const id of handOversLeft()) document.cookie = `${PREFIX}${id}=; Path=/; Max-Age=0`;
  Reflect.deleteProperty(navigator, "userAgent");
  Reflect.deleteProperty(navigator, "sendBeacon");
  Reflect.deleteProperty(window, "__DCL_PUBLIC__");
  window.history.replaceState(null, "", "/");
});

describe("a page a legacy redirect landed on", () => {
  it("sends Legacy Events Redirected once, after its page view, with the legacy properties", async () => {
    handOver(A, EVENTS_REDIRECT);
    await landOn(via("/events?tab=live", A), { userId: "0xAbC" });
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
    expect(handOversLeft()).toEqual([]);
  });

  it("removes the hand-over's fragment from the URL before the page view", async () => {
    handOver(A, EVENTS_REDIRECT);
    await landOn(via("/events?tab=live", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(2));
    expect(window.location.hash).toBe("");
    const [view] = await bodies();
    expect((view!.properties as { url: string }).url).not.toContain("dcl-legacy-redirect");
    expect((view!.context as { page: { url: string } }).page.url).not.toContain("dcl-legacy-redirect");
  });

  it("sends Legacy Places Redirected for a Discover URL, without a userId for an anonymous visitor", async () => {
    handOver(A, { ...PLACES_REDIRECT, destination: "/events" });
    await landOn(via("/events", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(2));
    const [event] = await redirectEvents();
    expect(event).toMatchObject({ event: "Legacy Places Redirected", properties: { origin: "places", source: "/discover" } });
    expect(event).not.toHaveProperty("userId");
  });

  it("gives two redirects to the same URL each to the navigation that followed it", async () => {
    handOver(A, EVENTS_REDIRECT);
    handOver(B, { ...EVENTS_REDIRECT, source: "/events/event?tab=live" });
    await landOn(via("/events?tab=live", B));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(2));
    expect((await redirectEvents()).map((e) => (e.properties as { source: string }).source)).toEqual(["/events/event?tab=live"]);
    expect(handOversLeft()).toEqual([A]);

    cleanup();
    sendBeacon.mockClear();
    await landOn(via("/events?tab=live", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(2));
    expect((await redirectEvents()).map((e) => (e.properties as { source: string }).source)).toEqual(["/whats-on?tab=live"]);
    expect(handOversLeft()).toEqual([]);
  });

  it("takes nothing when the destination URL is opened directly", async () => {
    handOver(A, EVENTS_REDIRECT);
    await landOn("/events?tab=live");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
    expect(handOversLeft()).toEqual([A]);
  });

  it("sends nothing for a fragment whose hand-over does not exist", async () => {
    await landOn(via("/events?tab=live", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
  });

  it("sends nothing, and clears it, when the hand-over named a different URL", async () => {
    handOver(A, EVENTS_REDIRECT);
    await landOn(via("/events?tab=upcoming", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
    expect(handOversLeft()).toEqual([]);
  });

  it("sends nothing for a bot, as the legacy SDK never loaded for one", async () => {
    Object.defineProperty(navigator, "userAgent", { value: BOT_UA, configurable: true });
    handOver(A, EVENTS_REDIRECT);
    await landOn(via("/events?tab=live", A));
    await waitFor(() => expect(handOversLeft()).toEqual([]));
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it("sends nothing in a session that started on an exempt page", async () => {
    handOver(A, EVENTS_REDIRECT);
    await landOn(via("/events?tab=live", A), { documentPath: "/download" });
    await waitFor(() => expect(handOversLeft()).toEqual([]));
    expect(sendBeacon).not.toHaveBeenCalled();
  });

  it("clears a malformed hand-over without sending it", async () => {
    handOver(A, { ...EVENTS_REDIRECT, origin: "blog" });
    await landOn(via("/events?tab=live", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
    expect(handOversLeft()).toEqual([]);
  });

  it("clears a hand-over whose params are not all strings without sending it", async () => {
    handOver(A, { ...EVENTS_REDIRECT, preservedParams: { tab: 1 } });
    await landOn(via("/events?tab=live", A));
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
    expect(handOversLeft()).toEqual([]);
  });

  it("sends only its page view when no redirect led to it", async () => {
    await landOn("/events");
    await waitFor(() => expect(sendBeacon).toHaveBeenCalledTimes(1));
    expect(await redirectEvents()).toEqual([]);
  });
});

// `/places` sends no root page view, so readiness is driven by hand: the legacy 800 ms window from landing.
describe("the legacy 800 ms window", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"] });
    vi.setSystemTime(NOW);
  });

  const flush = () => act(() => vi.advanceTimersByTimeAsync(0));

  async function landed() {
    handOver(A, PLACES_REDIRECT);
    const landing = await landOn(via("/places", A));
    await flush();
    expect(handOversLeft()).toEqual([]);
    return landing;
  }

  async function sentEvents() {
    return (await redirectEvents()).map((e) => e.event);
  }

  it("sends the event when analytics becomes ready within 800 ms of landing", async () => {
    const { page } = await landed();
    await act(() => vi.advanceTimersByTimeAsync(700));
    page("/places", {}, { sid: "sid-1" });
    expect(await sentEvents()).toEqual(["Legacy Places Redirected"]);
  });

  it("sends nothing when analytics is still not ready 800 ms after landing", async () => {
    const { page } = await landed();
    await act(() => vi.advanceTimersByTimeAsync(800));
    page("/places", {}, { sid: "sid-1" });
    expect(await sentEvents()).toEqual([]);
  });

  it("keeps the window measured from landing when the root re-renders with another account", async () => {
    const { page, router } = await landed();
    await act(() => vi.advanceTimersByTimeAsync(500));
    rootUser = "0xabc";
    await act(() => router.revalidate());
    await act(() => vi.advanceTimersByTimeAsync(400));
    page("/places", {}, { sid: "sid-1" });
    expect(await sentEvents()).toEqual([]);
  });

  it("still sends within the window after such a re-render, naming the new account", async () => {
    const { page, router } = await landed();
    await act(() => vi.advanceTimersByTimeAsync(300));
    rootUser = "0xabc";
    await act(() => router.revalidate());
    page("/places", {}, { sid: "sid-1" });
    const [event] = await redirectEvents();
    expect(event).toMatchObject({ event: "Legacy Places Redirected", userId: "0xabc" });
  });

  it("sends nothing when the visitor leaves before analytics is ready", async () => {
    const { page, router } = await landed();
    await act(() => router.navigate("/help"));
    page("/places", {}, { sid: "sid-1" });
    expect(await sentEvents()).toEqual([]);
  });

  it("sends nothing when the visitor leaves and comes back to the landing entry", async () => {
    const { page, router } = await landed();
    await act(() => router.navigate("/help"));
    await act(() => router.navigate(-1));
    await flush();
    expect(router.state.location.pathname).toBe("/places");
    page("/places", {}, { sid: "sid-1" });
    await act(() => vi.advanceTimersByTimeAsync(100));
    expect(await sentEvents()).toEqual([]);
  });

  it("sends nothing after it expired, even once the visitor comes back", async () => {
    const { page, router } = await landed();
    await act(() => vi.advanceTimersByTimeAsync(900));
    await act(() => router.navigate("/help"));
    await act(() => router.navigate(-1));
    await flush();
    page("/places", {}, { sid: "sid-1" });
    expect(await sentEvents()).toEqual([]);
  });

  it("sends it once, never again on a later return to the landing entry", async () => {
    const { page, router } = await landed();
    page("/places", {}, { sid: "sid-1" });
    await act(() => router.navigate("/help"));
    await act(() => router.navigate(-1));
    await flush();
    expect(await sentEvents()).toEqual(["Legacy Places Redirected"]);
  });
});
