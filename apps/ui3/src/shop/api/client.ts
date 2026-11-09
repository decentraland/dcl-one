// The Shop API client: the endpoint table over a transport, signed by a signer, at a base. All of them are given:
// the client reads no configuration and reaches for no network or identity of its own. The browser host's wiring
// (where the API is, fetch, who signs, the document's client) is api/host.ts.

import { SHOP_API_PREFIX, templateParams, type Endpoint } from "./endpoints";
import type { ShopSigner, ShopTransport, TransportRequest } from "./signer";

export type ShopApiMode = "canonical" | "legacy";

export type QueryParams = Record<string, string | number | boolean | null | undefined | (string | number)[]>;

export class ShopApiError extends Error {
  override name = "ShopApiError";
  constructor(
    message: string,
    readonly status: number,
    readonly url: string,
    readonly body: unknown = null,
    readonly code: string = "",
  ) {
    super(message);
  }
}

export function isShopApiError(e: unknown): e is ShopApiError {
  return e instanceof ShopApiError || (e as { name?: string } | null)?.name === "ShopApiError";
}

export type RequestInput = {
  params?: Record<string, string | number>;
  query?: QueryParams;
  body?: unknown;
  signal?: AbortSignal;
  metadata?: Record<string, unknown>;
  headers?: Record<string, string>;
};

export type ShopApiOptions = {
  /** Where the API is, as the host resolved and normalized it (no trailing slash). */
  base: string;
  mode: ShopApiMode;
  /** Who signs the non-public routes; none, and a signed route fails before the transport. */
  signer?: ShopSigner | null | (() => ShopSigner | null | Promise<ShopSigner | null>);
  transport: ShopTransport;
};

export type ShopApi = {
  readonly mode: ShopApiMode;
  readonly base: string;
  path(endpoint: Endpoint, params?: RequestInput["params"]): string;
  url(endpoint: Endpoint, input?: Pick<RequestInput, "params" | "query">): string;
  request<T>(endpoint: Endpoint, input?: RequestInput): Promise<T>;
};

function buildQuery(query?: QueryParams): string {
  if (!query) return "";
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") continue;
    if (Array.isArray(value)) for (const item of value) params.append(key, String(item));
    else params.set(key, String(value));
  }
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

function fill(template: string, params: RequestInput["params"] = {}): string {
  for (const name of templateParams(template)) {
    if (params[name] === undefined || params[name] === "") throw new TypeError(`missing path parameter {${name}} for ${template}`);
  }
  return template.replace(/\{([A-Za-z]+)\}/g, (_, name: string) => encodeURIComponent(String(params[name])));
}

function parseBody(text: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

export function createShopApi(opts: ShopApiOptions): ShopApi {
  const { mode, base, transport } = opts;
  const resolveSigner = async () => (typeof opts.signer === "function" ? await opts.signer() : (opts.signer ?? null));

  const api: ShopApi = {
    mode,
    base,
    path(endpoint, params) {
      if (mode === "legacy") {
        if (!endpoint.legacy) {
          throw new ShopApiError(
            `${endpoint.method} ${SHOP_API_PREFIX}${endpoint.path} has no local legacy alias`,
            0,
            endpoint.path,
            null,
            "noLegacyAlias",
          );
        }
        return fill(endpoint.legacy, params);
      }
      return SHOP_API_PREFIX + fill(endpoint.path, params);
    },
    url(endpoint, input = {}) {
      return `${base}${api.path(endpoint, input.params)}${buildQuery(input.query)}`;
    },
    async request<T>(endpoint: Endpoint, input: RequestInput = {}): Promise<T> {
      const url = api.url(endpoint, input);
      const headers: Record<string, string> = { accept: "application/json", ...(input.headers ?? {}) };
      const hasBody = input.body !== undefined && input.body !== null;
      if (hasBody) headers["content-type"] = "application/json";
      const signer = endpoint.auth === "public" ? null : await resolveSigner();
      if (endpoint.auth === "signed" && !signer) {
        throw new ShopApiError("Sign in to continue.", 401, url, null, "signInRequired");
      }
      const signPath = new URL(url, "http://shop.invalid").pathname;
      if (signer) Object.assign(headers, await signer.sign(endpoint.method, signPath, input.metadata ?? {}));
      const req: TransportRequest = {
        url,
        method: endpoint.method,
        headers,
        body: hasBody ? JSON.stringify(input.body) : undefined,
        signal: input.signal,
        signed: !!signer,
        signPath,
      };
      let res: { status: number; body: string };
      try {
        res = await transport(req);
      } catch (err) {
        if ((err as { name?: string } | null)?.name === "AbortError") throw err;
        throw new ShopApiError(`Shop request failed: ${(err as Error)?.message ?? "network error"}`, 0, url, null, "network");
      }
      const body = parseBody(res.body);
      if (res.status < 200 || res.status >= 300) {
        const serverError =
          body && typeof body === "object" ? ((body as { error?: string; message?: string }).error ?? (body as { message?: string }).message) : undefined;
        const message = serverError || (typeof body === "string" && body) || `Shop API returned ${res.status}`;
        throw new ShopApiError(message, res.status, url, body, serverError ?? "");
      }
      return body as T;
    },
  };
  return api;
}

export function insufficientCredits(e: unknown): { balanceCents: number; requiredCents: number } | null {
  if (!isShopApiError(e) || e.status !== 402) return null;
  const body = e.body as { balanceCents?: unknown; requiredCents?: unknown } | null;
  return body && typeof body.balanceCents === "number" && typeof body.requiredCents === "number"
    ? { balanceCents: body.balanceCents, requiredCents: body.requiredCents }
    : null;
}

export function creditsDisabled(e: unknown): boolean {
  return isShopApiError(e) && e.status === 404 && e.code === "USD credits are not enabled";
}

export function tooManyLiveCredits(e: unknown): boolean {
  return isShopApiError(e) && e.status === 409;
}
