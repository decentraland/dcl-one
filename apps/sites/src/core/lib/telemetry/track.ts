import { isbot } from "isbot";

import type { TelemetryEventName, TelemetryEvents } from "./events";

/** The app a message belongs to: it selects the write key and the browser gates. */
export type TelemetrySource = "sites" | "shop";

export type TrackContext = {
  sid: string;
  story?: string;
  variant?: string;
  /** The signed-in user's id (a wallet address), sent lowercased as Segment's `userId`. */
  userId?: string;
  /** Unset means `sites`. */
  source?: TelemetrySource;
  /**
   * A Sites conversion (a download) skips the bot and exempt-session gates, as the legacy site's
   * beacon did. The Shop suppressed every bot event, purchases included, so it is ignored there.
   */
  conversion?: boolean;
  /**
   * A legacy-parity event (a name the legacy site sent): it follows the legacy exempt-session gate,
   * as `page` and `identify` always do. Events without it are never held back by that gate.
   */
  legacy?: boolean;
};

export type TrackFn = <K extends TelemetryEventName>(
  event: K,
  props: TelemetryEvents[K],
  ctx: TrackContext,
) => void;

// Segment's HTTP Tracking API takes the public `writeKey` in the JSON body
// (no Authorization header), which is what lets `navigator.sendBeacon` post to it.
const DEFAULT_SEGMENT_API = "https://api.segment.io/v1";
export const DEFAULT_SEGMENT_TRACK_URL = `${DEFAULT_SEGMENT_API}/track`;
const LIBRARY = { name: "dcl-sites-beacon", version: "1.0.0" } as const;
/** A slow endpoint holds neither a browser socket nor a server-rendered request past this. */
const SEND_TIMEOUT_MS = 5000;
/**
 * The events the server sends a minute, in three windows: its 404s, its reports (schema
 * drift, which the asset catalog reports through too) and every other event (relayed client
 * errors, the page events loaders emit). A scanner's requests are not visitors, each one
 * without a cookie would be a new Segment user, and neither a scanner with a browser's user
 * agent nor an unauthenticated client-error post may starve the reports.
 */
const SERVER_EVENTS_PER_MINUTE = 60;
const NOT_FOUND_EVENT = "page_not_found";
const REPORT_EVENTS = new Set(["catalyst_schema_drift"]);
const serverBudgets = {
  notFound: { windowStart: 0, sent: 0 },
  reports: { windowStart: 0, sent: 0 },
  other: { windowStart: 0, sent: 0 },
};

function serverBudgetAllows(event: string, now = Date.now()): boolean {
  const budget =
    event === NOT_FOUND_EVENT
      ? serverBudgets.notFound
      : REPORT_EVENTS.has(event)
        ? serverBudgets.reports
        : serverBudgets.other;
  // A clock behind the window's start (set back, or a test's) opens a new window too.
  if (now - budget.windowStart >= 60_000 || now < budget.windowStart) {
    budget.windowStart = now;
    budget.sent = 0;
  }
  budget.sent += 1;
  return budget.sent <= SERVER_EVENTS_PER_MINUTE;
}

/**
 * Query parameters that carry a credential (`cast-stream?token=` takes a stream key, an
 * OAuth return carries `code`, `/auth/callback?authResult=` the session token, Magic's
 * return `magic_credential`): dropped from the page URL, the referrer, a 404's path and a
 * relayed client error's URL before they reach Segment or a log, and from a query-shaped
 * fragment too (the scene editor takes the SDK run's token as `#token=`). Every other
 * parameter, `utm_*` included, is kept.
 */
const SENSITIVE_QUERY_PARAMS = new Set([
  "token",
  "code",
  "key",
  "signature",
  "access_token",
  "authresult",
  "magic_credential",
  // The e-mail confirmation pages carry the wallet in the query: a wallet reaches Segment only as `userId`.
  "address",
]);

function withoutSensitivePairs(part: string): string {
  return part
    .split("&")
    .filter((pair) => {
      if (pair === "") return false;
      const eq = pair.indexOf("=");
      let name = pair.slice(0, eq < 0 ? pair.length : eq);
      try {
        name = decodeURIComponent(name);
      } catch {
      }
      return !SENSITIVE_QUERY_PARAMS.has(name.toLowerCase());
    })
    .join("&");
}

/** Path segments that are credentials: a Cast stream link's streaming key (`/cast/s/{key}`) is
 * all `POST /cast/streamer-token` checks, and an e-mail confirmation link's code confirms the
 * address it was sent to. */
const SENSITIVE_PATHS: Array<[RegExp, string]> = [
  // Routes match case-insensitively, so `/Cast/S/{key}` serves the same page.
  [/\/cast\/s\/[^/?#]+/gi, "/cast/s/:key"],
  [/\/account\/(confirm-email-challenge|confirm-email|credits-email-confirmed)\/[^/?#]+/gi, "/account/$1/:code"],
];

export function withoutSensitivePath(path: string): string {
  let out = path;
  for (const [pattern, replacement] of SENSITIVE_PATHS) out = out.replace(pattern, replacement);
  return out;
}

export function withoutSensitiveParams(raw: string): string {
  const url = withoutSensitivePath(raw);
  const h = url.indexOf("#");
  const beforeHash = h < 0 ? url : url.slice(0, h);
  const fragment = h < 0 ? "" : url.slice(h + 1);
  const q = beforeHash.indexOf("?");
  const query = q < 0 ? "" : beforeHash.slice(q + 1);
  const keptQuery = withoutSensitivePairs(query);
  const keptFragment = fragment.includes("=") ? withoutSensitivePairs(fragment) : fragment;
  if (keptQuery === query && keptFragment === fragment) return url;
  const base = q < 0 ? beforeHash : beforeHash.slice(0, q);
  return base + (keptQuery ? `?${keptQuery}` : "") + (keptFragment ? `#${keptFragment}` : "");
}

function env(name: string): string | undefined {
  try {
    return typeof process !== "undefined" ? process.env?.[name] : undefined;
  } catch {
    return undefined;
  }
}

function publicEnv(name: string): string | undefined {
  try {
    const e = (import.meta as unknown as { env?: Record<string, string> }).env;
    if (e) return e[`VITE_${name}`] ?? e[name];
  } catch {
  }
  return undefined;
}

function runtimeEnv(name: string): string | undefined {
  if (typeof window === "undefined") return undefined;
  const pub = (window as unknown as { __DCL_PUBLIC__?: Record<string, string> })
    .__DCL_PUBLIC__;
  const v = pub?.[name];
  return v && v !== "" ? v : undefined;
}

/**
 * Fills the Sites transport keys `window.__DCL_PUBLIC__` lacks (a document rendered without the root
 * loader publishes them empty) from values the server answered; a key already set is kept.
 */
export function adoptSitesSegmentConfig(config: { segmentWriteKey?: string; segmentApiHost?: string }): void {
  if (typeof window === "undefined") return;
  const w = window as unknown as { __DCL_PUBLIC__?: Record<string, string> };
  const pub = (w.__DCL_PUBLIC__ ??= {});
  if (!pub.SEGMENT_WRITE_KEY && config.segmentWriteKey) pub.SEGMENT_WRITE_KEY = config.segmentWriteKey;
  if (!pub.SEGMENT_API_HOST && config.segmentApiHost) pub.SEGMENT_API_HOST = config.segmentApiHost;
}

function setting(
  name: "SEGMENT_WRITE_KEY" | "SHOP_SEGMENT_WRITE_KEY" | "SEGMENT_API_HOST",
): string | undefined {
  const v = typeof window === "undefined"
    ? env(name) ?? publicEnv(name)
    : runtimeEnv(name) ?? publicEnv(name);
  return v && v !== "" ? v : undefined;
}

/**
 * The public Segment write key of a source (config, not a secret: it ships in the bundle): Sites
 * reads `SEGMENT_WRITE_KEY`, the Shop its own `SHOP_SEGMENT_WRITE_KEY`, with no fallback between
 * them. Unset, that source's messages are a no-op.
 */
export function segmentWriteKey(source: TelemetrySource = "sites"): string | undefined {
  return setting(source === "shop" ? "SHOP_SEGMENT_WRITE_KEY" : "SEGMENT_WRITE_KEY");
}

/**
 * A message type's endpoint: Segment's own, or the first-party proxy `SEGMENT_API_HOST`
 * names as `host/basePath` (a protocol is accepted and stripped).
 */
export function segmentApiUrl(type: SegmentBody["type"]): string {
  const host = setting("SEGMENT_API_HOST")
    ?.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
    .replace(/\/+$/, "");
  return `${host ? `https://${host}` : DEFAULT_SEGMENT_API}/${type}`;
}

export function segmentTrackUrl(): string {
  return segmentApiUrl("track");
}

const isBrowser = (): boolean => typeof window !== "undefined";

type SegmentIdentity = { anonymousId: string; userId?: string };

export type SegmentTrackBody = SegmentIdentity & {
  type: "track";
  event: string;
  properties: Record<string, unknown> & {
    story?: string;
    variant?: string;
  };
};

export type SegmentPageBody = SegmentIdentity & {
  type: "page";
  name: string;
  properties: Record<string, unknown>;
};

export type SegmentIdentifyBody = SegmentIdentity & {
  type: "identify";
  userId: string;
  traits: Record<string, unknown>;
};

export type SegmentBody = SegmentTrackBody | SegmentPageBody | SegmentIdentifyBody;

type SegmentPage = { url: string; path: string; referrer: string; search: string; title: string };
type SegmentCampaign = { name?: string; source?: string; medium?: string; term?: string; content?: string };

export type SegmentContext = {
  direct?: true;
  page?: SegmentPage;
  campaign?: SegmentCampaign;
  userAgent?: string;
  locale?: string;
  timezone?: string;
  library: typeof LIBRARY;
};

export type SegmentPayload<B extends SegmentBody = SegmentBody> = B & {
  writeKey: string;
  integrations: Record<string, never>;
  messageId: string;
  timestamp: string;
  sentAt: string;
  context: SegmentContext;
};

export type SegmentTrackPayload = SegmentPayload<SegmentTrackBody>;

function identity(ctx: TrackContext): SegmentIdentity {
  return { anonymousId: ctx.sid, ...(ctx.userId ? { userId: ctx.userId.toLowerCase() } : {}) };
}

export function buildSegmentBody(
  event: string,
  props: Record<string, unknown>,
  ctx: TrackContext,
): SegmentTrackBody {
  return {
    type: "track",
    event,
    ...identity(ctx),
    properties: {
      ...props,
      story: ctx.story,
      ...(ctx.variant !== undefined ? { variant: ctx.variant } : {}),
    },
  };
}

function randomId(): string {
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return crypto.randomUUID();
    }
  } catch {
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function messageId(): string {
  return `${LIBRARY.name}-${randomId()}`;
}

function timezone(): string | undefined {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || undefined;
  } catch {
    return undefined;
  }
}

/** The browser's page, as analytics.js reports it in `context.page` and a page call's properties. */
function currentPage(): SegmentPage {
  return {
    url: withoutSensitiveParams(window.location.href),
    path: withoutSensitivePath(window.location.pathname),
    referrer: withoutSensitiveParams(typeof document !== "undefined" ? document.referrer : ""),
    search: withoutSensitiveParams(window.location.search ?? ""),
    title: typeof document !== "undefined" ? document.title : "",
  };
}

const CAMPAIGN_FIELDS = [
  ["utm_campaign", "name"],
  ["utm_source", "source"],
  ["utm_medium", "medium"],
  ["utm_term", "term"],
  ["utm_content", "content"],
] as const;

/** Segment's `context.campaign`, from the URL's `utm_*` parameters; none present, none sent. */
function campaign(search: string): SegmentCampaign | undefined {
  const params = new URLSearchParams(search);
  const out: SegmentCampaign = {};
  for (const [param, field] of CAMPAIGN_FIELDS) {
    const value = params.get(param);
    if (value) out[field] = value;
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

/** Segment stamps `context.ip` only for a request marked `direct`, so the browser sets it and SSR does not. */
function context(): SegmentContext {
  if (!isBrowser()) return { library: LIBRARY };
  const tz = timezone();
  const page = currentPage();
  const utm = campaign(page.search);
  return {
    direct: true,
    page,
    ...(utm ? { campaign: utm } : {}),
    userAgent: typeof navigator !== "undefined" ? navigator.userAgent : "",
    locale: typeof navigator !== "undefined" ? navigator.language : "",
    ...(tz ? { timezone: tz } : {}),
    library: LIBRARY,
  };
}

export function buildSegmentPayload<B extends SegmentBody>(
  body: B,
  writeKey: string,
  now: Date = new Date(),
): SegmentPayload<B> {
  const at = now.toISOString();
  const integrations: Record<string, never> = {};
  return {
    ...body,
    writeKey,
    integrations,
    messageId: messageId(),
    timestamp: at,
    sentAt: at,
    context: context(),
  };
}

/** The classifier the legacy apps' analytics provider used (`isbot`). */
export function isBotUserAgent(userAgent: string): boolean {
  return isbot(userAgent);
}

/**
 * Pages a visitor reaches from a shared link, where a session that starts sends no legacy-parity
 * message: the legacy site's exempt set (`/download` and every legal document but `/credits-terms`),
 * at every path the site serves each document on (its slug, its key, an alias). A test in
 * `data/legal` keeps this list equal to the served legal paths.
 */
const EXEMPT_PATHS = new Set([
  "/download",
  "/brand",
  "/content",
  "/content-policy",
  "/ethics",
  "/rewards-terms",
  "/rewards",
  "/creator-fund-terms",
  "/creatorfund",
  "/security",
  "/privacy",
  "/referral-terms",
  "/referral",
  "/terms",
]);

export function isAnalyticsExemptPath(pathname: string): boolean {
  const path = (pathname.length > 1 && pathname.endsWith("/") ? pathname.slice(0, -1) : pathname).toLowerCase();
  return EXEMPT_PATHS.has(path);
}

/** The path this document was loaded on: a client navigation does not change it, a reload does. */
let documentInitialPath: string | undefined;

function initialPath(): string {
  if (documentInitialPath === undefined) {
    let path = window.location?.pathname ?? "";
    try {
      const [entry] = performance.getEntriesByType("navigation");
      if (entry?.name) path = new URL(entry.name).pathname;
    } catch {
    }
    documentInitialPath = path;
  }
  return documentInitialPath;
}

/**
 * Decided once per loaded document, from the path it was loaded on (the navigation entry, so a
 * module loaded after a client navigation still sees it): navigating to an exempt page later keeps
 * the session tracked, leaving the exempt page it started on keeps it untracked, and a reload
 * decides again.
 */
function exemptSession(): boolean {
  return isAnalyticsExemptPath(initialPath());
}

type GateContext = Pick<TrackContext, "source" | "conversion" | "legacy">;

/**
 * The browser gates: a bot, for every source; and, for Sites only, an exempt session, which holds
 * back `page`, `identify` and legacy-parity tracks, never the site's other events.
 */
function suppressedInBrowser(body: SegmentBody, ctx: GateContext): boolean {
  const sites = (ctx.source ?? "sites") === "sites";
  if (sites && ctx.conversion) return false;
  if (isBotUserAgent(typeof navigator !== "undefined" ? navigator.userAgent ?? "" : "")) return true;
  const legacyParity = body.type !== "track" || ctx.legacy === true;
  return sites && legacyParity && exemptSession();
}

function sendToSegment(body: SegmentBody, ctx: GateContext): void {
  const writeKey = segmentWriteKey(ctx.source);
  if (!writeKey) return;
  if (isBrowser() && suppressedInBrowser(body, ctx)) return;

  const url = segmentApiUrl(body.type);
  const payload = JSON.stringify(buildSegmentPayload(body, writeKey));

  if (isBrowser()) {
    try {
      // text/plain keeps the request CORS-simple; application/json would preflight,
      // which is unsafe during unload.
      const beacon = navigator?.sendBeacon?.bind(navigator);
      if (beacon && beacon(url, new Blob([payload], { type: "text/plain" }))) return;
    } catch {
    }
    postTelemetry(url, payload, "text/plain");
    return;
  }

  if (body.type !== "track" || !serverBudgetAllows(body.event)) return;
  postTelemetry(url, payload, "application/json");
}

/** `AbortSignal.timeout` is Safari 16+: before it, an `AbortController` holds the deadline. */
function deadline(ms: number): AbortSignal | undefined {
  try {
    if (typeof AbortSignal.timeout === "function") return AbortSignal.timeout(ms);
    const controller = new AbortController();
    setTimeout(() => controller.abort(), ms);
    return controller.signal;
  } catch {
    return undefined;
  }
}

function postTelemetry(url: string, payload: string, contentType: string): void {
  try {
    void fetch(url, {
      method: "POST",
      headers: { "Content-Type": contentType },
      body: payload,
      keepalive: true,
      mode: "cors",
      credentials: "omit",
      signal: deadline(SEND_TIMEOUT_MS),
    })
      .then((response) => response.body?.cancel())
      .catch(() => {});
  } catch {
  }
}

function devValidate(event: string, props: Record<string, unknown>): void {
  const env = (import.meta as { env?: { DEV?: boolean; TEST?: boolean } }).env;
  if (!env?.DEV || env.TEST) return;
  void import("./dev-validate")
    .then((m) => m.warnIfInvalid(event, props))
    .catch(() => {});
}

/** The Sites event contract does not cover the Shop's event names, so only Sites events are validated. */
function sendEvent(event: string, props: Record<string, unknown>, ctx: TrackContext): void {
  try {
    if ((ctx.source ?? "sites") === "sites") devValidate(event, props);
    sendToSegment(buildSegmentBody(event, props, ctx), ctx);
  } catch {
  }
}

export function track<K extends TelemetryEventName>(
  event: K,
  props: TelemetryEvents[K],
  ctx: TrackContext,
): void {
  sendEvent(event, props as Record<string, unknown>, ctx);
}

/** A caller's `url`, `path`, `referrer` or `search` cannot bring a credential back into a page call. */
function sanitizedPageProperties(properties: Record<string, unknown>): Record<string, unknown> {
  const out = { ...properties };
  for (const key of ["url", "referrer", "search"] as const) {
    if (typeof out[key] === "string") out[key] = withoutSensitiveParams(out[key]);
  }
  if (typeof out.path === "string") out.path = withoutSensitivePath(out.path);
  return out;
}

/**
 * A Segment page call, with the properties analytics.js adds (`path`, `url`, `referrer`, `search`,
 * `title`) under the caller's. Browser only: a server render is not a page view.
 */
export function page(name: string, properties: Record<string, unknown>, ctx: TrackContext): void {
  try {
    if (!isBrowser()) return;
    sendToSegment(
      {
        type: "page",
        name: withoutSensitivePath(name),
        ...identity(ctx),
        properties: sanitizedPageProperties({ ...currentPage(), ...properties }),
      },
      ctx,
    );
  } catch {
  }
}

/**
 * A Segment identify call for `userId` (lowercased). Browser only. Traits go to Segment as given:
 * never pass personal data (e-mail, name, phone) as a trait.
 */
export function identify(userId: string, traits: Record<string, unknown>, ctx: TrackContext): void {
  try {
    if (!isBrowser() || !userId) return;
    sendToSegment(
      { type: "identify", anonymousId: ctx.sid, userId: userId.toLowerCase(), traits },
      ctx,
    );
  } catch {
  }
}

/** The client a caller outside React holds (a store, an imperative flow). */
export type TelemetryClient = {
  readonly anonymousId: string;
  /** Sends with this client's identity; it has no story, variant or conversion. */
  track(event: string, props?: Record<string, unknown>): void;
  /** Never pass personal data (e-mail, name, phone) as a trait. */
  identify(userId: string, traits?: Record<string, unknown>): void;
  /**
   * Forgets the user and starts a new anonymous id, returned so the caller can persist it. It
   * changes this client only: the HttpOnly `sid` cookie, and every Sites `track` sent with it,
   * keeps the old id, so the visitor appears under both ids from then on.
   */
  reset(): string;
};

/**
 * A browser client with its own identity: nothing is shared between instances or module-wide, so
 * nothing leaks between server requests, where every method is a no-op. It writes no cookie or
 * storage; the caller persists `anonymousId` if it wants it back on the next creation.
 */
export function createTelemetryClient(opts: {
  source: TelemetrySource;
  anonymousId: string;
  userId?: string;
}): TelemetryClient {
  let anonymousId = opts.anonymousId;
  let userId = opts.userId?.toLowerCase();
  const ctx = (): TrackContext => ({ sid: anonymousId, source: opts.source, ...(userId ? { userId } : {}) });
  return {
    get anonymousId() {
      return anonymousId;
    },
    track(event, props = {}) {
      if (isBrowser()) sendEvent(event, props, ctx());
    },
    identify(id, traits = {}) {
      if (!isBrowser() || !id) return;
      userId = id.toLowerCase();
      identify(userId, traits, ctx());
    },
    reset() {
      if (!isBrowser()) return anonymousId;
      userId = undefined;
      anonymousId = randomId();
      return anonymousId;
    },
  };
}
