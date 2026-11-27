import { siteBase } from "../site";

export class CatalystError extends Error {
  override name = "CatalystError";
  status: number;
  url: string;
  /** Seconds the server asked to wait before retrying (`Retry-After` or a JSON `retryAfter`). */
  retryAfterSecs?: number;
  /** The JSON body's `code`, when the server names the refusal. */
  code?: string;

  constructor(message: string, url: string, status = 0) {
    super(message);
    this.status = status;
    this.url = url;
  }
}

export type QueryParams = Record<string, string | number | boolean | null | undefined | (string | number)[]>;

export type RequestOpts = {
  base?: string;
  service?: ServiceName;
  query?: QueryParams;
  signal?: AbortSignal;
  headers?: Record<string, string>;
  fetchImpl?: typeof fetch;
};

export type SendOpts = RequestOpts & {
  method?: string;
  body?: unknown;
  timeoutMs?: number;
};

type CatalystWindow = Window & typeof globalThis & { __CATALYST_BASE__?: string };

function errMessage(err: unknown, fallback: string): string {
  return (err as { message?: string } | null)?.message ?? fallback;
}

/** The server's message in a parsed error envelope: `message` where a host still
 * sends it, else `error` (the places and events routes answer `{ok, error, code}`). */
export function envelopeMessage(parsed: unknown): string {
  const body = parsed as { message?: unknown; error?: unknown } | null;
  if (typeof body?.message === "string") return body.message;
  if (typeof body?.error === "string") return body.error;
  return "";
}

function defaultBase(): string {
  if (!import.meta.env?.SSR) return window.location.origin;
  const proc = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process;
  return proc?.env?.CATALYST_URL || "http://127.0.0.1:5210";
}

export function catalystBase(override?: string): string {
  const fromEnv: string | undefined =
    (typeof import.meta !== "undefined" ? import.meta.env?.VITE_CATALYST_URL : undefined) || undefined;
  const fromWindow =
    typeof window !== "undefined" ? (window as CatalystWindow).__CATALYST_BASE__ : undefined;
  const base = override ?? fromEnv ?? fromWindow ?? defaultBase();
  return base.replace(/\/$/, "");
}

type ServiceName =
  | "places"
  | "events"
  | "communities"
  | "communitiesCdn"
  | "notifications"
  | "badges"
  | "cameraReel"
  | "map"
  | "satellite"
  | "market";

const SERVICE_PATHS: Record<ServiceName, string> = {
  places: "/places",
  events: "/events",
  communities: "",
  communitiesCdn: "",
  notifications: "/notifications",
  badges: "",
  cameraReel: "/camera-reel",
  map: "",
  satellite: "/satellite",
  market: "/market",
};

// One static read per key: Vite inlines `import.meta.env.X` as that value alone, but a computed
// `import.meta.env[key]` as the whole env object, every build-time `VITE_*` value, into the bundle.
function serviceEnv(service: ServiceName): string | undefined {
  if (typeof import.meta === "undefined") return undefined;
  const env: Record<ServiceName, string | undefined> = {
    places: import.meta.env?.VITE_PLACES_URL,
    events: import.meta.env?.VITE_EVENTS_URL,
    communities: import.meta.env?.VITE_COMMUNITIES_URL,
    communitiesCdn: import.meta.env?.VITE_COMMUNITIES_CDN_URL,
    notifications: import.meta.env?.VITE_NOTIFICATIONS_URL,
    badges: import.meta.env?.VITE_BADGES_URL,
    cameraReel: import.meta.env?.VITE_CAMERA_REEL_URL,
    map: import.meta.env?.VITE_MAP_URL,
    satellite: import.meta.env?.VITE_SATELLITE_URL,
    market: import.meta.env?.VITE_MARKET_URL,
  };
  return env[service];
}

type ServicesWindow = Window &
  typeof globalThis & {
    __SERVICE_BASES__?: Partial<Record<ServiceName, string>>;
    __DCL_PLAY_EMBEDDED__?: boolean;
  };

export function serviceBase(service: ServiceName, override?: string): string {
  const fromEnv = serviceEnv(service);
  const fromWindow =
    typeof window !== "undefined"
      ? (window as ServicesWindow).__SERVICE_BASES__?.[service]
      : undefined;
  const embedded =
    typeof window !== "undefined" && (window as ServicesWindow).__DCL_PLAY_EMBEDDED__ === true;
  const base = override ?? fromEnv ?? fromWindow ?? `${embedded ? siteBase() : defaultBase()}${SERVICE_PATHS[service]}`;
  return base.replace(/\/$/, "");
}

export function serviceHref(service: ServiceName, override?: string): string {
  const fromEnv = serviceEnv(service);
  const fromWindow =
    typeof window !== "undefined"
      ? (window as ServicesWindow).__SERVICE_BASES__?.[service]
      : undefined;
  const embedded =
    typeof window !== "undefined" && (window as ServicesWindow).__DCL_PLAY_EMBEDDED__ === true;
  const base = override ?? fromEnv ?? fromWindow ?? `${embedded ? siteBase() : ""}${SERVICE_PATHS[service]}`;
  return base.replace(/\/$/, "");
}

export function servicePath(service: ServiceName, override?: string): string {
  const fromEnv = serviceEnv(service);
  const base = override ?? fromEnv ?? SERVICE_PATHS[service];
  return base.replace(/\/$/, "");
}

function resolveBase(opts: RequestOpts): string {
  if (opts.base != null) return catalystBase(opts.base);
  if (opts.service) return serviceBase(opts.service);
  return catalystBase();
}

function buildQuery(query?: QueryParams): string {
  if (!query) return "";
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") continue;
    if (Array.isArray(value)) {
      for (const item of value) params.append(key, String(item));
      continue;
    }
    params.set(key, String(value));
  }
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

/** The URL a request goes to: its base (`opts.base`, its service's, or the node's), the path and the query. */
export function requestUrl(path: string, opts: RequestOpts = {}): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  return `${resolveBase(opts)}${cleanPath}${buildQuery(opts.query)}`;
}

export async function getJSON<T = unknown>(path: string, opts: RequestOpts = {}): Promise<T> {
  const url = requestUrl(path, opts);
  const doFetch = opts.fetchImpl ?? fetch;

  let res: Response;
  try {
    res = await doFetch(url, {
      signal: opts.signal,
      headers: { accept: "application/json", ...(opts.headers ?? {}) },
    });
  } catch (err) {
    throw new CatalystError(
      `Catalyst request failed: ${errMessage(err, "network error")}`,
      url,
    );
  }

  if (!res.ok) {
    throw new CatalystError(
      `Catalyst returned ${res.status} ${res.statusText}`,
      url,
      res.status,
    );
  }

  try {
    return (await res.json()) as T;
  } catch (err) {
    throw new CatalystError(
      `Catalyst returned invalid JSON: ${errMessage(err, "parse error")}`,
      url,
      res.status,
    );
  }
}

export async function sendJSON<T = unknown>(path: string, opts: SendOpts = {}): Promise<T | null> {
  const url = requestUrl(path, opts);
  const doFetch = opts.fetchImpl ?? fetch;
  const method = (opts.method ?? "POST").toUpperCase();
  const hasBody = opts.body !== undefined && opts.body !== null;

  let res: Response;
  try {
    res = await doFetch(url, {
      method,
      signal: opts.signal,
      headers: {
        accept: "application/json",
        ...(hasBody ? { "content-type": "application/json" } : {}),
        ...(opts.headers ?? {}),
      },
      body: hasBody ? JSON.stringify(opts.body) : undefined,
    });
  } catch (err) {
    throw new CatalystError(
      `Catalyst request failed: ${errMessage(err, "network error")}`,
      url,
    );
  }

  if (!res.ok) {
    let serverMsg = "";
    try {
      const txt = await res.text();
      if (txt) {
        try {
          serverMsg = envelopeMessage(JSON.parse(txt));
        } catch {
          serverMsg = txt;
        }
      }
    } catch {
    }
    throw new CatalystError(
      serverMsg || `Catalyst returned ${res.status} ${res.statusText}`,
      url,
      res.status,
    );
  }

  if (res.status === 204) return null;
  let text: string;
  try {
    text = await res.text();
  } catch {
    return null;
  }
  if (!text) return null;
  try {
    return JSON.parse(text) as T;
  } catch {
    return text as unknown as T;
  }
}

/**
 * The error for a failed answer: its status, the message of its JSON envelope (or its text), the `retryAfter` it
 * repeats in the body (a bridge passes no headers) and its body `code`.
 */
export function forwardFailure(url: string, status: number, body: string): CatalystError {
  let serverMsg = "";
  let retryAfter: unknown;
  let code: unknown;
  if (body) {
    try {
      const parsed = JSON.parse(body) as { retryAfter?: unknown; code?: unknown } | null;
      serverMsg = envelopeMessage(parsed);
      retryAfter = parsed?.retryAfter;
      code = parsed?.code;
    } catch {
      serverMsg = body;
    }
  }
  const err = new CatalystError(serverMsg || `Catalyst returned ${status}`, url, status);
  if (typeof retryAfter === "number" && Number.isFinite(retryAfter) && retryAfter > 0) err.retryAfterSecs = retryAfter;
  if (typeof code === "string") err.code = code;
  return err;
}
