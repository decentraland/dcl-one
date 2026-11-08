import type { TelemetryEventName, TelemetryEvents } from "./events";

export type TrackContext = {
  sid: string;
  story?: string;
  variant?: string;
};

export type TrackFn = <K extends TelemetryEventName>(
  event: K,
  props: TelemetryEvents[K],
  ctx: TrackContext,
) => void;

// Segment's HTTP Tracking API takes the public `writeKey` in the JSON body
// (no Authorization header), which is what lets `navigator.sendBeacon` post to it.
export const DEFAULT_SEGMENT_TRACK_URL = "https://api.segment.io/v1/track";
const LIBRARY = { name: "dcl-sites-beacon", version: "1.0.0" } as const;
/** A slow endpoint holds neither a browser socket nor a server-rendered request past this. */
const SEND_TIMEOUT_MS = 5000;
/**
 * The events the server sends a minute (its 404s and error reports): a scanner's requests
 * are not visitors, and each one without a cookie would be a new Segment user.
 */
const SERVER_EVENTS_PER_MINUTE = 60;
let serverWindowStart = 0;
let serverWindowSent = 0;

function serverBudgetAllows(now = Date.now()): boolean {
  if (now - serverWindowStart >= 60_000) {
    serverWindowStart = now;
    serverWindowSent = 0;
  }
  serverWindowSent += 1;
  return serverWindowSent <= SERVER_EVENTS_PER_MINUTE;
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

function setting(name: "SEGMENT_WRITE_KEY" | "SEGMENT_API_HOST"): string | undefined {
  const v = typeof window === "undefined"
    ? env(name) ?? publicEnv(name)
    : runtimeEnv(name) ?? publicEnv(name);
  return v && v !== "" ? v : undefined;
}

/** The public Segment write key (config, not a secret: it ships in the bundle). Unset, `track` is a no-op. */
export function segmentWriteKey(): string | undefined {
  return setting("SEGMENT_WRITE_KEY");
}

/**
 * The track endpoint: Segment's own, or the first-party proxy `SEGMENT_API_HOST`
 * names as `host/basePath` (a protocol is accepted and stripped).
 */
export function segmentTrackUrl(): string {
  const host = setting("SEGMENT_API_HOST")
    ?.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
    .replace(/\/+$/, "");
  return host ? `https://${host}/track` : DEFAULT_SEGMENT_TRACK_URL;
}

const isBrowser = (): boolean => typeof window !== "undefined";

export type SegmentTrackBody = {
  type: "track";
  event: string;
  anonymousId: string;
  properties: Record<string, unknown> & {
    story?: string;
    variant?: string;
  };
};

export type SegmentContext = {
  direct?: true;
  page?: { url: string; path: string; referrer: string; title: string };
  userAgent?: string;
  locale?: string;
  timezone?: string;
  library: typeof LIBRARY;
};

export type SegmentTrackPayload = SegmentTrackBody & {
  writeKey: string;
  integrations: Record<string, never>;
  messageId: string;
  timestamp: string;
  sentAt: string;
  context: SegmentContext;
};

export function buildSegmentBody(
  event: string,
  props: Record<string, unknown>,
  ctx: TrackContext,
): SegmentTrackBody {
  return {
    type: "track",
    event,
    anonymousId: ctx.sid,
    properties: {
      ...props,
      story: ctx.story,
      ...(ctx.variant !== undefined ? { variant: ctx.variant } : {}),
    },
  };
}

function messageId(): string {
  let random = "";
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      random = crypto.randomUUID();
    }
  } catch {
  }
  if (!random) random = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `${LIBRARY.name}-${random}`;
}

function timezone(): string | undefined {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || undefined;
  } catch {
    return undefined;
  }
}

/** Segment stamps `context.ip` only for a request marked `direct`, so the browser sets it and SSR does not. */
function context(): SegmentContext {
  if (!isBrowser()) return { library: LIBRARY };
  const tz = timezone();
  return {
    direct: true,
    page: {
      url: window.location.href,
      path: window.location.pathname,
      referrer: typeof document !== "undefined" ? document.referrer : "",
      title: typeof document !== "undefined" ? document.title : "",
    },
    userAgent: typeof navigator !== "undefined" ? navigator.userAgent : "",
    locale: typeof navigator !== "undefined" ? navigator.language : "",
    ...(tz ? { timezone: tz } : {}),
    library: LIBRARY,
  };
}

export function buildSegmentPayload(
  body: SegmentTrackBody,
  writeKey: string,
  now: Date = new Date(),
): SegmentTrackPayload {
  const at = now.toISOString();
  return {
    writeKey,
    ...body,
    integrations: {},
    messageId: messageId(),
    timestamp: at,
    sentAt: at,
    context: context(),
  };
}

function sendToSegment(body: SegmentTrackBody): void {
  const writeKey = segmentWriteKey();
  if (!writeKey) return;

  const url = segmentTrackUrl();
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

  if (!serverBudgetAllows()) return;
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

export function track<K extends TelemetryEventName>(
  event: K,
  props: TelemetryEvents[K],
  ctx: TrackContext,
): void {
  try {
    devValidate(event, props as Record<string, unknown>);
    sendToSegment(buildSegmentBody(event, props, ctx));
  } catch {
  }
}
