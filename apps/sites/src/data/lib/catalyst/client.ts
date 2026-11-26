import type { AuthIdentity, SignedFetchMetadata } from "../auth/types";
import { catalystUrl, worldsUrl } from "./bases";

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
    const body = JSON.parse(await res.text()) as { message?: unknown; error?: unknown } | null;
    // `message` where a host still sends it; the places and events routes answer
    // the old services' `{ok, error, code}` envelope.
    const msg = typeof body?.message === "string" ? body.message : body?.error;
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

export type QueryValue = string | number | boolean | readonly string[] | undefined | null;
export type Query = Record<string, QueryValue>;

export function buildQuery(query?: Query): string {
  if (!query) return "";
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") continue;
    if (Array.isArray(value)) {
      for (const item of value) if (item) params.append(key, item);
    } else {
      params.set(key, String(value));
    }
  }
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

export type GetOptions = {
  base?: string;
  query?: Query;
  signal?: AbortSignal;
  fetchImpl?: typeof fetch;
  /** Retain the server message for endpoints with a structured error contract. */
  readErrorBody?: boolean;
  cache?: RequestCache;
  headers?: Record<string, string>;
};

type Fetched = { status: number; text: string };

const inflightByRequest = new WeakMap<AbortSignal, Map<string, Promise<Fetched>>>();

async function fetchText(
  url: string,
  init: RequestInit,
  doFetch: typeof fetch,
  readErrorBody: boolean,
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
    if (readErrorBody) throw await errorFromResponse(res, url);
    await res.body?.cancel().catch(() => {});
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
  readErrorBody: boolean,
): Promise<Fetched> {
  if (!signal) return fetchText(url, init, doFetch, readErrorBody);
  let perRequest = inflightByRequest.get(signal);
  if (!perRequest) {
    perRequest = new Map();
    inflightByRequest.set(signal, perRequest);
  }
  const key = `${url}\0${init.cache ?? ""}\0${JSON.stringify(init.headers ?? {})}\0${readErrorBody}`;
  const pending = perRequest.get(key);
  if (pending) return pending;
  const map = perRequest;
  const p = fetchText(url, init, doFetch, readErrorBody).finally(() => {
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
    opts.readErrorBody ?? false,
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
  method?: "POST" | "PUT" | "PATCH" | "DELETE";
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

/** An unsigned JSON `POST` for the node's public batch reads (`/lambdas/profiles`, `/content/entities/active`). */
export async function postPublicJSON(
  path: string,
  body: unknown,
  opts: GetOptions & { allowEmpty?: boolean } = {},
): Promise<unknown> {
  const url = `${catalystBase(opts.base)}${path}`;
  const { text } = await fetchText(
    url,
    {
      method: "POST",
      headers: { accept: "application/json", "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: opts.signal,
    },
    opts.fetchImpl ?? fetch,
    true,
  );
  return opts.allowEmpty && !text ? undefined : JSON.parse(text);
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
