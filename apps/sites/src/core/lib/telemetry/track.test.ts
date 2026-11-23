import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  DEFAULT_SEGMENT_TRACK_URL,
  buildSegmentBody,
  buildSegmentPayload,
  createTelemetryClient,
  identify,
  isBotUserAgent,
  page,
  segmentApiUrl,
  segmentTrackUrl,
  track,
  type TrackContext,
  withoutSensitiveParams,
  withoutSensitivePath,
} from "./track";

const CTX: TrackContext = {
  sid: "sid-123",
  story: "homepage-hero",
  variant: "B",
};

const SEGMENT_BODY = {
  type: "track",
  event: "cta_clicked",
  anonymousId: "sid-123",
  properties: {
    surface: "hero",
    story: "homepage-hero",
    variant: "B",
  },
};

const trackAny = track as unknown as (
  event: string,
  props: Record<string, unknown>,
  ctx: TrackContext,
) => void;

describe("track (fetch mocked)", () => {
  const fetchMock = vi.fn(() => Promise.resolve(new Response(null, { status: 200 })));

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockClear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete process.env.SEGMENT_WRITE_KEY;
    delete process.env.SEGMENT_API_HOST;
  });

  it("preserves an event variant when the context does not override it", () => {
    expect(buildSegmentBody("cast_ending", { variant: "base" }, { sid: "test" }).properties.variant).toBe("base");
    expect(buildSegmentBody("cast_ending", { variant: "base" }, { sid: "test", variant: "genesis" }).properties.variant).toBe("genesis");
  });

  it("wraps the body in Segment's HTTP API envelope: the public write key, a message id, timestamps and the library context", () => {
    const now = new Date("2026-11-07T12:00:00.000Z");
    const payload = buildSegmentPayload(buildSegmentBody("cta_clicked", { surface: "hero" }, CTX), "key-1", now);
    expect(payload).toMatchObject({
      ...SEGMENT_BODY,
      writeKey: "key-1",
      integrations: {},
      timestamp: "2026-11-07T12:00:00.000Z",
      sentAt: "2026-11-07T12:00:00.000Z",
      context: { library: { name: "dcl-sites-beacon", version: "1.0.0" } },
    });
    expect(payload.messageId).toMatch(/^dcl-sites-beacon-/);
    expect(payload.context.direct).toBeUndefined();
  });

  it("POSTs the Segment body to the track endpoint without an Authorization header", async () => {
    expect(buildSegmentBody("cta_clicked", { surface: "hero" }, CTX)).toEqual(SEGMENT_BODY);
    process.env.SEGMENT_WRITE_KEY = "key-1";
    trackAny("cta_clicked", { surface: "hero" }, CTX);
    await Promise.resolve();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe(DEFAULT_SEGMENT_TRACK_URL);
    expect(init.method).toBe("POST");
    const headers = init.headers as Record<string, string>;
    expect(headers.Authorization).toBeUndefined();
    expect(headers["Content-Type"]).toBe("application/json");
    const sent = JSON.parse(init.body as string);
    expect(sent).toMatchObject({ ...SEGMENT_BODY, writeKey: "key-1" });
    expect(typeof sent.messageId).toBe("string");
    expect(init.signal).toBeInstanceOf(AbortSignal);
    expect(init.signal?.aborted).toBe(false);
  });

  it("bounds the request with a timeout signal and cancels the response body it does not read", async () => {
    process.env.SEGMENT_WRITE_KEY = "key-1";
    const cancel = vi.fn(() => Promise.resolve());
    fetchMock.mockImplementationOnce(() => Promise.resolve({ body: { cancel } } as unknown as Response));
    trackAny("evt", {}, CTX);
    await vi.waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(init.signal).toBeInstanceOf(AbortSignal);
  });

  it("keeps the deadline on a browser without AbortSignal.timeout (Safari before 16)", () => {
    process.env.SEGMENT_WRITE_KEY = "key-1";
    const signals = AbortSignal as unknown as { timeout?: unknown };
    const timeout = signals.timeout;
    vi.useFakeTimers();
    try {
      signals.timeout = undefined;
      trackAny("evt", {}, CTX);
      expect(fetchMock).toHaveBeenCalledTimes(1);
      const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
      expect(init.signal).toBeInstanceOf(AbortSignal);
      expect(init.signal?.aborted).toBe(false);
      vi.advanceTimersByTime(5000);
      expect(init.signal?.aborted).toBe(true);
    } finally {
      signals.timeout = timeout;
      vi.useRealTimers();
    }
  });

  it("sends the browser page and referrer as the browser has them, less the credential parameters", async () => {
    vi.stubGlobal("window", {
      location: {
        href: "https://sites.example/shop/items?utm_source=x&code=abc&q=secret#top",
        pathname: "/shop/items",
      },
      __DCL_PUBLIC__: { SEGMENT_WRITE_KEY: "key-1" },
    });
    vi.stubGlobal("document", { referrer: "https://ref.example/from?token=private#x", title: "Items" });
    vi.stubGlobal("navigator", { userAgent: BROWSER_UA, language: "en" });
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect((init.headers as Record<string, string>)["Content-Type"]).toBe("text/plain");
    expect(init.signal).toBeInstanceOf(AbortSignal);
    const sent = JSON.parse(init.body as string);
    expect(sent.context.direct).toBe(true);
    expect(sent.context.page).toEqual({
      url: "https://sites.example/shop/items?utm_source=x&q=secret#top",
      path: "/shop/items",
      referrer: "https://ref.example/from#x",
      search: "",
      title: "Items",
    });
  });

  it("never sends a Cast streaming key, which is the stream link's path", async () => {
    vi.stubGlobal("window", {
      location: {
        href: "https://sites.example/cast/s/live-key-123?step=x",
        pathname: "/cast/s/live-key-123",
      },
      __DCL_PUBLIC__: { SEGMENT_WRITE_KEY: "key-1" },
    });
    vi.stubGlobal("document", { referrer: "https://sites.example/cast/s/live-key-123", title: "Cast" });
    vi.stubGlobal("navigator", { userAgent: BROWSER_UA, language: "en" });
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    const body = init.body as string;
    expect(body).not.toContain("live-key-123");
    expect(JSON.parse(body).context.page).toMatchObject({
      url: "https://sites.example/cast/s/:key?step=x",
      path: "/cast/s/:key",
      referrer: "https://sites.example/cast/s/:key",
    });
    expect(withoutSensitiveParams("/cast/w/10,10")).toBe("/cast/w/10,10");
  });

  it("drops only the credential parameters, in the query or a query-shaped fragment, in any position or case, and leaves a path without any alone", () => {
    expect(withoutSensitiveParams("/cast-stream?token=abc")).toBe("/cast-stream");
    expect(withoutSensitiveParams("/a?x=1&Access_Token=t&y=2#h")).toBe("/a?x=1&y=2#h");
    expect(withoutSensitiveParams("/a?%74oken=t&utm_medium=m")).toBe("/a?utm_medium=m");
    expect(withoutSensitiveParams("/a?signature&key=")).toBe("/a");
    expect(withoutSensitiveParams("/auth/callback?authResult=%7B%7D&next=%2Fshop")).toBe("/auth/callback?next=%2Fshop");
    expect(withoutSensitiveParams("/auth/callback?magic_credential=m&state=s")).toBe("/auth/callback?state=s");
    expect(withoutSensitiveParams("/a?token=x&")).toBe("/a");
    expect(withoutSensitiveParams("/a?token=x&&y=1")).toBe("/a?y=1");
    expect(withoutSensitiveParams("/a?q=token#token=1")).toBe("/a?q=token");
    expect(withoutSensitiveParams("/create/preview?project=p#token=abc&x=1")).toBe("/create/preview?project=p#x=1");
    expect(withoutSensitiveParams("/a#top")).toBe("/a#top");
    expect(withoutSensitiveParams("/a?#token")).toBe("/a?#token");
    expect(withoutSensitiveParams("")).toBe("");
  });

  it("budgets server 404s, reports and other events apart, 60 a minute each, and resumes with the next minute", async () => {
    process.env.SEGMENT_WRITE_KEY = "key-1";
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date("2030-01-01T00:00:00.000Z"));
      for (let i = 0; i < 61; i += 1) trackAny("page_not_found", {}, CTX);
      await Promise.resolve();
      expect(fetchMock).toHaveBeenCalledTimes(60);
      for (let i = 0; i < 61; i += 1) trackAny("catalyst_schema_drift", {}, CTX);
      await Promise.resolve();
      expect(fetchMock).toHaveBeenCalledTimes(120);
      trackAny("client_error", {}, CTX);
      await Promise.resolve();
      expect(fetchMock).toHaveBeenCalledTimes(121);
      vi.setSystemTime(new Date("2030-01-01T00:01:00.000Z"));
      trackAny("page_not_found", {}, CTX);
      trackAny("catalyst_schema_drift", {}, CTX);
      await Promise.resolve();
      expect(fetchMock).toHaveBeenCalledTimes(123);
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens a new window when the clock is behind the window's start", async () => {
    process.env.SEGMENT_WRITE_KEY = "key-1";
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date("2030-06-01T00:00:00.000Z"));
      for (let i = 0; i < 61; i += 1) trackAny("page_not_found", {}, CTX);
      await Promise.resolve();
      expect(fetchMock).toHaveBeenCalledTimes(60);
      vi.setSystemTime(new Date("2030-05-31T23:59:30.000Z"));
      trackAny("page_not_found", {}, CTX);
      await Promise.resolve();
      expect(fetchMock).toHaveBeenCalledTimes(61);
    } finally {
      vi.useRealTimers();
    }
  });

  it("posts through navigator.sendBeacon in a browser and falls back to fetch when the beacon is refused", async () => {
    vi.stubGlobal("window", {
      location: { href: "https://sites.example/", pathname: "/" },
      __DCL_PUBLIC__: { SEGMENT_WRITE_KEY: "key-1" },
    });
    vi.stubGlobal("document", { referrer: "", title: "" });
    const sendBeacon = vi.fn(() => true);
    vi.stubGlobal("navigator", { userAgent: BROWSER_UA, language: "en", sendBeacon });
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    expect(sendBeacon).toHaveBeenCalledTimes(1);
    const [url, blob] = sendBeacon.mock.calls[0] as unknown as [string, Blob];
    expect(url).toBe(DEFAULT_SEGMENT_TRACK_URL);
    expect(blob.type).toBe("text/plain");
    expect(JSON.parse(await blob.text()).writeKey).toBe("key-1");
    expect(fetchMock).not.toHaveBeenCalled();

    sendBeacon.mockReturnValue(false);
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    expect(sendBeacon).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect((init.headers as Record<string, string>)["Content-Type"]).toBe("text/plain");
  });

  it("posts to the first-party proxy when SEGMENT_API_HOST is set, with or without a protocol or trailing slash", async () => {
    process.env.SEGMENT_WRITE_KEY = "key-1";
    process.env.SEGMENT_API_HOST = "api.e.example.org/v1/";
    expect(segmentTrackUrl()).toBe("https://api.e.example.org/v1/track");
    process.env.SEGMENT_API_HOST = "https://api.e.example.org/v1";
    expect(segmentTrackUrl()).toBe("https://api.e.example.org/v1/track");
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    const [url] = fetchMock.mock.calls[0] as unknown as [string];
    expect(url).toBe("https://api.e.example.org/v1/track");
  });

  it("never throws: an unset write key skips fetch, a throwing fetch is swallowed, no ctx env is a no-op", async () => {
    expect(process.env.SEGMENT_WRITE_KEY).toBeUndefined();
    expect(() => trackAny("evt", { a: 1 }, CTX)).not.toThrow();
    expect(() => trackAny("evt", { x: 1 }, { sid: "anon" })).not.toThrow();
    await Promise.resolve();
    expect(fetchMock).not.toHaveBeenCalled();
    process.env.SEGMENT_WRITE_KEY = "key-1";
    fetchMock.mockImplementationOnce(() => {
      throw new Error("network down");
    });
    expect(() => trackAny("evt", {}, CTX)).not.toThrow();
    await Promise.resolve();
  });
});

const BROWSER_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const GOOGLEBOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
const BOT_UAS = [
  GOOGLEBOT_UA,
  "curl/8.6.0",
  "python-requests/2.32.3",
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/120.0.0.0 Safari/537.36",
  "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Chrome-Lighthouse",
];
const HUMAN_UAS = [
  BROWSER_UA,
  "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
  "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
  "Mozilla/5.0 (Linux; Android 12; CUBOT P50) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36",
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0",
];

/** A fresh module, as a newly loaded document has: its exempt decision is not the previous test's. */
async function loadDocument() {
  vi.resetModules();
  return import("./track");
}

type BrowserOptions = {
  path?: string;
  search?: string;
  userAgent?: string;
  publicEnv?: Record<string, string>;
  /** The document's navigation entry, when it differs from the current location. */
  navigationUrl?: string;
};

/** A browser on `path + search`; returns its location, which a test mutates to navigate. */
function stubBrowser({
  path = "/",
  search = "",
  userAgent = BROWSER_UA,
  publicEnv = { SEGMENT_WRITE_KEY: "key-1" },
  navigationUrl,
}: BrowserOptions = {}) {
  const location = { href: `https://sites.example${path}${search}`, pathname: path, search };
  const entryUrl = navigationUrl ?? location.href;
  vi.stubGlobal("performance", { getEntriesByType: (type: string) => (type === "navigation" ? [{ name: entryUrl }] : []) });
  vi.stubGlobal("window", { location, __DCL_PUBLIC__: publicEnv });
  vi.stubGlobal("document", { referrer: "https://ref.example/", title: "Title" });
  vi.stubGlobal("navigator", { userAgent, language: "en" });
  return {
    navigate(to: string) {
      location.pathname = to;
      location.href = `https://sites.example${to}`;
      location.search = "";
    },
  };
}

type Sent = { url: string; body: Record<string, any> };

function sent(fetchMock: { mock: { calls: unknown[][] } }): Sent[] {
  return fetchMock.mock.calls.map((call) => {
    const [url, init] = call as unknown as [string, RequestInit];
    return { url, body: JSON.parse(init.body as string) };
  });
}

describe("page, identify, gates and the telemetry client (fetch mocked)", () => {
  const fetchMock = vi.fn(() => Promise.resolve(new Response(null, { status: 200 })));

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockClear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete process.env.SEGMENT_WRITE_KEY;
    delete process.env.SHOP_SEGMENT_WRITE_KEY;
    delete process.env.SEGMENT_API_HOST;
  });

  it("posts a page call with analytics.js's page properties, sanitised, under the caller's", () => {
    stubBrowser({ path: "/explore", search: "?utm_source=x&token=secret&q=1" });
    page("Explore", { section: "map", title: "Custom" }, { sid: "sid-1" });
    const [{ url, body }] = sent(fetchMock);
    expect(url).toBe("https://api.segment.io/v1/page");
    expect(body).toMatchObject({
      type: "page",
      name: "Explore",
      anonymousId: "sid-1",
      writeKey: "key-1",
      integrations: {},
      properties: {
        path: "/explore",
        url: "https://sites.example/explore?utm_source=x&q=1",
        referrer: "https://ref.example/",
        search: "?utm_source=x&q=1",
        title: "Custom",
        section: "map",
      },
    });
    expect(JSON.stringify(body)).not.toContain("secret");
    expect(body.messageId).toMatch(/^dcl-sites-beacon-/);
    expect(body.timestamp).toBe(body.sentAt);
    expect(body.context.page.search).toBe("?utm_source=x&q=1");
    expect(body).not.toHaveProperty("userId");
  });

  it("posts an identify call with traits and the user id lowercased", () => {
    stubBrowser();
    identify("0xABCDEF", { plan: "free" }, { sid: "sid-1" });
    const [{ url, body }] = sent(fetchMock);
    expect(url).toBe("https://api.segment.io/v1/identify");
    expect(body).toMatchObject({ type: "identify", anonymousId: "sid-1", userId: "0xabcdef", traits: { plan: "free" } });
  });

  it("sends page and identify to the first-party proxy when SEGMENT_API_HOST is set", () => {
    stubBrowser({ publicEnv: { SEGMENT_WRITE_KEY: "key-1", SEGMENT_API_HOST: "https://api.e.example.org/v1/" } });
    expect(segmentApiUrl("page")).toBe("https://api.e.example.org/v1/page");
    page("Home", {}, { sid: "s" });
    identify("0x1", {}, { sid: "s" });
    expect(sent(fetchMock).map((m) => m.url)).toEqual([
      "https://api.e.example.org/v1/page",
      "https://api.e.example.org/v1/identify",
    ]);
  });

  it("sends the user id lowercased on track, and none when the context has none", () => {
    expect(buildSegmentBody("evt", {}, { sid: "s", userId: "0xABC" }).userId).toBe("0xabc");
    expect(buildSegmentBody("evt", {}, { sid: "s" })).not.toHaveProperty("userId");
    stubBrowser();
    page("Home", {}, { sid: "s", userId: "0xDEF" });
    expect(sent(fetchMock)[0].body.userId).toBe("0xdef");
  });

  it("sends Segment's campaign context from the utm parameters, and none without them", () => {
    stubBrowser({ search: "?utm_campaign=launch&utm_source=x&utm_medium=social&utm_term=t&utm_content=c&q=1" });
    trackAny("evt", {}, CTX);
    expect(sent(fetchMock)[0].body.context.campaign).toEqual({
      name: "launch",
      source: "x",
      medium: "social",
      term: "t",
      content: "c",
    });
    fetchMock.mockClear();
    stubBrowser({ search: "?q=1" });
    trackAny("evt", {}, CTX);
    expect(sent(fetchMock)[0].body.context).not.toHaveProperty("campaign");
  });

  it("sends neither page nor identify without a write key, nor from the server", () => {
    stubBrowser({ publicEnv: {} });
    page("Home", {}, { sid: "s" });
    identify("0x1", {}, { sid: "s" });
    expect(fetchMock).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
    vi.stubGlobal("fetch", fetchMock);
    process.env.SEGMENT_WRITE_KEY = "key-1";
    page("Home", {}, { sid: "s" });
    identify("0x1", {}, { sid: "s" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("drops a bot's events, unless they are Sites conversions", () => {
    expect(isBotUserAgent(GOOGLEBOT_UA)).toBe(true);
    for (const ua of BOT_UAS) expect(isBotUserAgent(ua), ua).toBe(true);
    for (const ua of HUMAN_UAS) expect(isBotUserAgent(ua), ua).toBe(false);
    stubBrowser({ userAgent: GOOGLEBOT_UA });
    trackAny("evt", {}, CTX);
    page("Home", {}, { sid: "s" });
    identify("0x1", {}, { sid: "s" });
    expect(fetchMock).not.toHaveBeenCalled();
    trackAny("download_started", {}, { ...CTX, conversion: true });
    expect(sent(fetchMock).map((m) => m.body.event)).toEqual(["download_started"]);
  });

  it("drops a Shop bot's events even when they are conversions", () => {
    stubBrowser({ userAgent: GOOGLEBOT_UA, publicEnv: { SHOP_SEGMENT_WRITE_KEY: "shop-key" } });
    trackAny("Shop Purchased", {}, { sid: "s", source: "shop", conversion: true });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("keeps a document loaded on an exempt page from sending legacy-parity messages after it navigates, except conversions", async () => {
    const browser = stubBrowser({ path: "/privacy/" });
    const fresh = await loadDocument();
    const freshTrack = fresh.track as unknown as typeof trackAny;
    fresh.page("Privacy", {}, { sid: "s" });
    browser.navigate("/explore");
    freshTrack("Click", {}, { ...CTX, legacy: true });
    fresh.page("Explore", {}, { sid: "s" });
    fresh.identify("0x1", {}, { sid: "s" });
    expect(fetchMock).not.toHaveBeenCalled();
    freshTrack("download_started", {}, { ...CTX, conversion: true, legacy: true });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("never holds back the site's own events in an exempt session", async () => {
    stubBrowser({ path: "/terms" });
    const fresh = await loadDocument();
    (fresh.track as unknown as typeof trackAny)("lp_legal_viewed", { doc: "terms" }, CTX);
    expect(sent(fetchMock).map((m) => m.body.event)).toEqual(["lp_legal_viewed"]);
  });

  it.each(["/content-policy", "/rewards", "/creatorFund", "/referral", "/Terms/", "/download"])(
    "treats %s, a served path of an exempt document, as exempt",
    async (path) => {
      stubBrowser({ path });
      (await loadDocument()).page("Doc", {}, { sid: "s" });
      expect(fetchMock).not.toHaveBeenCalled();
    },
  );

  it("does not treat /credits-terms, which the legacy site tracked, as exempt", async () => {
    stubBrowser({ path: "/credits-terms" });
    (await loadDocument()).page("Credits", {}, { sid: "s" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("keeps tracking a document loaded elsewhere after it navigates to an exempt page", async () => {
    const browser = stubBrowser({ path: "/" });
    const fresh = await loadDocument();
    browser.navigate("/privacy");
    fresh.page("Privacy", {}, { sid: "s" });
    expect(sent(fetchMock).map((m) => m.url)).toEqual(["https://api.segment.io/v1/page"]);
  });

  it("decides again on a reload: exempt to tracked, and tracked to exempt", async () => {
    stubBrowser({ path: "/terms" });
    (await loadDocument()).page("Terms", {}, { sid: "s" });
    expect(fetchMock).not.toHaveBeenCalled();
    stubBrowser({ path: "/explore" });
    (await loadDocument()).page("Explore", {}, { sid: "s" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    stubBrowser({ path: "/terms" });
    (await loadDocument()).page("Terms", {}, { sid: "s" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("takes the document's path from its navigation entry when the module loads after a client navigation", async () => {
    stubBrowser({ path: "/explore", navigationUrl: "https://sites.example/privacy" });
    (await loadDocument()).page("Explore", {}, { sid: "s" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("does not apply the exempt-page gate to the Shop", async () => {
    stubBrowser({ path: "/privacy", publicEnv: { SHOP_SEGMENT_WRITE_KEY: "shop-key" } });
    const fresh = await loadDocument();
    (fresh.track as unknown as typeof trackAny)("Shop Viewed Page", {}, { sid: "s", source: "shop" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("redacts credential paths in any case and with a trailing slash, and the wallet in a confirmation link's query", () => {
    expect(withoutSensitivePath("/CAST/S/live-key/")).toBe("/cast/s/:key/");
    expect(withoutSensitivePath("/Account/Confirm-Email/CODE-1/")).toBe("/account/Confirm-Email/:code/");
    const referrer = "https://sites.example/account/confirm-email/CODE-2?address=0xabc&utm_source=mail";
    const clean = withoutSensitiveParams(referrer);
    expect(clean).toBe("https://sites.example/account/confirm-email/:code?utm_source=mail");
    expect(clean).not.toContain("CODE-2");
    expect(clean).not.toContain("0xabc");
  });

  it("never sends an e-mail confirmation code, from the browser's URL or from a caller's page properties", async () => {
    const code = "SENTINEL-c0de-1234";
    for (const kind of ["confirm-email", "confirm-email-challenge", "credits-email-confirmed"]) {
      fetchMock.mockClear();
      const path = `/account/${kind}/${code}`;
      stubBrowser({ path, search: `?token=${code}` });
      vi.stubGlobal("document", { referrer: `https://sites.example${path}`, title: "Confirm" });
      const fresh = await loadDocument();
      fresh.page(path, { url: `https://sites.example${path}?code=${code}`, path, referrer: `https://sites.example${path}`, search: `?token=${code}` }, { sid: "s" });
      (fresh.track as unknown as typeof trackAny)("evt", { surface: "x" }, CTX);
      const calls = fetchMock.mock.calls as unknown as [string, RequestInit][];
      expect(calls).toHaveLength(2);
      for (const [, init] of calls) expect(init.body as string).not.toContain(code);
      expect(sent(fetchMock)[0].body.name).toBe(`/account/${kind}/:code`);
      expect(sent(fetchMock)[0].body.properties.path).toBe(`/account/${kind}/:code`);
    }
  });

  it("sends Shop events with the Shop's own key and never Sites'", () => {
    stubBrowser({ publicEnv: { SEGMENT_WRITE_KEY: "key-1", SHOP_SEGMENT_WRITE_KEY: "shop-key" } });
    trackAny("Shop Viewed Page", {}, { sid: "s", source: "shop" });
    expect(sent(fetchMock)[0].body.writeKey).toBe("shop-key");
    fetchMock.mockClear();
    stubBrowser({ publicEnv: { SEGMENT_WRITE_KEY: "key-1" } });
    trackAny("Shop Viewed Page", {}, { sid: "s", source: "shop" });
    expect(fetchMock).not.toHaveBeenCalled();
    process.env.SHOP_SEGMENT_WRITE_KEY = "shop-server-key";
    vi.unstubAllGlobals();
    vi.stubGlobal("fetch", fetchMock);
    trackAny("Shop Viewed Page", {}, { sid: "s", source: "shop" });
    expect(sent(fetchMock)[0].body.writeKey).toBe("shop-server-key");
  });

  describe("createTelemetryClient", () => {
    it("carries the identified user on later events, lowercased", () => {
      stubBrowser({ publicEnv: { SHOP_SEGMENT_WRITE_KEY: "shop-key" } });
      const client = createTelemetryClient({ source: "shop", anonymousId: "anon-1" });
      client.track("Shop Opened");
      client.identify("0xABC", { method: "wallet" });
      client.track("Shop Signed In", { step: 1 });
      const messages = sent(fetchMock);
      expect(messages.map((m) => m.url)).toEqual([
        "https://api.segment.io/v1/track",
        "https://api.segment.io/v1/identify",
        "https://api.segment.io/v1/track",
      ]);
      expect(messages[0].body).not.toHaveProperty("userId");
      expect(messages[1].body).toMatchObject({ userId: "0xabc", anonymousId: "anon-1", traits: { method: "wallet" } });
      expect(messages[2].body).toMatchObject({
        event: "Shop Signed In",
        userId: "0xabc",
        anonymousId: "anon-1",
        writeKey: "shop-key",
        properties: { step: 1 },
      });
    });

    it("forgets the user and rotates the anonymous id on reset, returning the new id", () => {
      stubBrowser();
      const client = createTelemetryClient({ source: "sites", anonymousId: "anon-1", userId: "0xAA" });
      expect(client.anonymousId).toBe("anon-1");
      const rotated = client.reset();
      expect(rotated).not.toBe("anon-1");
      expect(rotated.length).toBeGreaterThan(0);
      expect(client.anonymousId).toBe(rotated);
      client.track("evt");
      const [{ body }] = sent(fetchMock);
      expect(body.anonymousId).toBe(rotated);
      expect(body).not.toHaveProperty("userId");
    });

    it("keeps each instance's identity its own", () => {
      stubBrowser();
      const a = createTelemetryClient({ source: "sites", anonymousId: "anon-a" });
      const b = createTelemetryClient({ source: "sites", anonymousId: "anon-b" });
      a.identify("0xA");
      a.reset();
      b.track("evt");
      const last = sent(fetchMock).at(-1)!.body;
      expect(last.anonymousId).toBe("anon-b");
      expect(last).not.toHaveProperty("userId");
      expect(b.anonymousId).toBe("anon-b");
    });

    it("is a no-op on the server", () => {
      process.env.SEGMENT_WRITE_KEY = "key-1";
      const client = createTelemetryClient({ source: "sites", anonymousId: "anon-1" });
      client.track("evt");
      client.identify("0x1");
      expect(client.reset()).toBe("anon-1");
      expect(fetchMock).not.toHaveBeenCalled();
    });

    it("sends nothing for the Shop while its key is unset, even with Sites' key set", () => {
      stubBrowser({ publicEnv: { SEGMENT_WRITE_KEY: "key-1" } });
      const client = createTelemetryClient({ source: "shop", anonymousId: "anon-1" });
      client.track("Shop Opened");
      client.identify("0x1");
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });
});
