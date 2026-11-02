import type { AuthIdentity, SignedFetchMetadata } from "../auth/types";
import { catalystUrl, publicWorldsUrl, worldsUrl } from "./bases";

async function signedFetch(
  ...args: Parameters<typeof import("../auth/signer").signedFetch>
): Promise<Response> {
  const signer = await import("../auth/signer");
  return signer.signedFetch(...args);
}

export class CatalystError extends Error {
  readonly status: number;
  readonly url: string;
  readonly serverMessage: boolean;
  constructor(message: string, url: string, status = 0, serverMessage = false) {
    super(message);
    this.name = "CatalystError";
    this.status = status;
    this.url = url;
    this.serverMessage = serverMessage;
  }
}

export async function errorFromResponse(res: Response, url: string): Promise<CatalystError> {
  let server: string | null = null;
  try {
    const body: unknown = JSON.parse(await res.text());
    const msg = (body as { message?: unknown })?.message;
    if (typeof msg === "string" && msg.trim()) server = msg.trim();
  } catch {
  }
  return server
    ? new CatalystError(server, url, res.status, true)
    : new CatalystError(
        `Catalyst returned ${res.status} ${res.statusText}`,
        url,
        res.status,
      );
}

export function catalystBase(override?: string): string {
  return catalystUrl(override);
}

export function worldsBase(override?: string): string {
  return worldsUrl(override);
}

/** The worlds origin browsers can reach. `worldsBase()` may point the server at a loopback
    twin (WORLDS_URL); links and thumbnails that ship to the page must not. */
export function publicWorldsBase(): string {
  return publicWorldsUrl();
}

export type QueryValue = string | number | boolean | undefined | null;
export type Query = Record<string, QueryValue>;

export function buildQuery(query?: Query): string {
  if (!query) return "";
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") continue;
    params.set(key, String(value));
  }
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

export type GetOptions = {
  base?: string;
  query?: Query;
  signal?: AbortSignal;
  fetchImpl?: typeof fetch;
  cache?: RequestCache;
  headers?: Record<string, string>;
};

type Fetched = { status: number; text: string };

const inflightByRequest = new WeakMap<AbortSignal, Map<string, Promise<Fetched>>>();

async function fetchText(
  url: string,
  init: RequestInit,
  doFetch: typeof fetch,
): Promise<Fetched> {
  let res: Response;
  try {
    res = await doFetch(url, init);
  } catch (err) {
    throw new CatalystError(
      `Catalyst request failed: ${(err as Error)?.message ?? "network error"}`,
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
    return { status: res.status, text: await res.text() };
  } catch (err) {
    throw new CatalystError(
      `Catalyst returned invalid JSON: ${(err as Error)?.message ?? "parse error"}`,
      url,
      res.status,
    );
  }
}

// Identical GETs in flight under one request signal share a single upstream fetch.
function fetchShared(
  url: string,
  init: RequestInit,
  doFetch: typeof fetch,
  signal: AbortSignal | undefined,
): Promise<Fetched> {
  if (!signal) return fetchText(url, init, doFetch);
  let perRequest = inflightByRequest.get(signal);
  if (!perRequest) {
    perRequest = new Map();
    inflightByRequest.set(signal, perRequest);
  }
  const key = `${url}\0${init.cache ?? ""}\0${JSON.stringify(init.headers ?? {})}`;
  const pending = perRequest.get(key);
  if (pending) return pending;
  const map = perRequest;
  const p = fetchText(url, init, doFetch).finally(() => {
    map.delete(key);
  });
  map.set(key, p);
  return p;
}

export async function getJSON<T>(path: string, opts: GetOptions = {}): Promise<T> {
  const base = catalystBase(opts.base);
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  const url = `${base}${cleanPath}${buildQuery(opts.query)}`;
  const doFetch = opts.fetchImpl ?? fetch;

  const { status, text } = await fetchShared(
    url,
    {
      signal: opts.signal,
      cache: opts.cache ?? "no-store",
      headers: { accept: "application/json", ...(opts.headers ?? {}) },
    },
    doFetch,
    opts.signal,
  );

  try {
    return JSON.parse(text) as T;
  } catch (err) {
    throw new CatalystError(
      `Catalyst returned invalid JSON: ${(err as Error)?.message ?? "parse error"}`,
      url,
      status,
    );
  }
}

export type PostOptions = {
  identity: AuthIdentity;
  method?: "POST" | "PATCH" | "DELETE";
  base?: string;
  query?: Query;
  signal?: AbortSignal;
  headers?: Record<string, string>;
  metadata?: SignedFetchMetadata;
  signPath?: string;
};

export type SignedGetOptions = {
  identity: AuthIdentity;
  base?: string;
  query?: Query;
  signal?: AbortSignal;
  metadata?: SignedFetchMetadata;
  signPath?: string;
};

export async function signedGetJSON<T>(
  path: string,
  opts: SignedGetOptions,
): Promise<T> {
  const base = catalystBase(opts.base);
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  const url = `${base}${cleanPath}${buildQuery(opts.query)}`;

  let res: Response;
  try {
    res = await signedFetch(opts.identity, url, {
      method: "GET",
      signal: opts.signal,
      metadata: opts.metadata ?? {},
      signPath: opts.signPath,
      headers: { accept: "application/json" },
    });
  } catch (err) {
    throw new CatalystError(
      `Catalyst request failed: ${(err as Error)?.message ?? "network error"}`,
      url,
    );
  }
  if (!res.ok) {
    throw await errorFromResponse(res, url);
  }
  try {
    return (await res.json()) as T;
  } catch (err) {
    throw new CatalystError(
      `Catalyst returned invalid JSON: ${(err as Error)?.message ?? "parse error"}`,
      url,
      res.status,
    );
  }
}

export async function postJSON<T>(
  path: string,
  body: unknown,
  opts: PostOptions,
): Promise<T> {
  const base = catalystBase(opts.base);
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  const url = `${base}${cleanPath}${buildQuery(opts.query)}`;
  const method = opts.method ?? "POST";

  const headers: Record<string, string> = {
    accept: "application/json",
    ...(opts.headers ?? {}),
  };
  const init: RequestInit & { metadata?: SignedFetchMetadata; signPath?: string } = {
    method,
    headers,
    signal: opts.signal,
    metadata: opts.metadata ?? {},
    signPath: opts.signPath,
  };
  if (method !== "DELETE" && body !== undefined) {
    headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }

  let res: Response;
  try {
    res = await signedFetch(opts.identity, url, init);
  } catch (err) {
    throw new CatalystError(
      `Catalyst request failed: ${(err as Error)?.message ?? "network error"}`,
      url,
    );
  }

  if (!res.ok) {
    throw await errorFromResponse(res, url);
  }

  const text = await res.text();
  if (!text) return undefined as T;
  try {
    return JSON.parse(text) as T;
  } catch (err) {
    throw new CatalystError(
      `Catalyst returned invalid JSON: ${(err as Error)?.message ?? "parse error"}`,
      url,
      res.status,
    );
  }
}
