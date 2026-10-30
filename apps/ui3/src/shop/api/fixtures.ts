// Fixture-backed mocks for stories and tests: a ShopTransport that answers the credit-authorization
// routes (canonical or legacy path) from fixtures/credits-wire.json, which generate.mjs derives from
// the pinned credits-server wire cases. Only the shapes those cases pin are served; a request no case
// matches answers 501, never an invented body. `$any` values are filled by the rule the case names:
// the salt is a bytes32 from the cancel case, `expiresAt` is now + the handler's TTL in seconds, and a
// batch's `lines` echo the request lines with each charge rounded up to a whole credit.

import { http, HttpResponse } from "msw";

import { chargeForQuote, usdCents } from "../model/money";
import { ENDPOINTS } from "./endpoints";
import type { ShopTransport, TransportRequest } from "./signer";
import wire from "./fixtures/credits-wire.json";

export type WireRoute = "authorize" | "batch" | "cancel" | "submitted";

export type WireCase = {
  route: WireRoute;
  name: string;
  source: string;
  auth: string | null;
  state: unknown;
  request: { body?: unknown; rawBody?: string };
  status: number;
  body: unknown;
  bodyAsserted: boolean;
};

export const WIRE_CASES = wire.cases as WireCase[];
/** The signed-fetch address every fixture case authenticates as. */
export const FIXTURE_BUYER = "0x8cb8e16ea85793c0da573615248b6c91c88df2dd";
const SALT = "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const DEFAULT_TTL_SECONDS = 300;

const ROUTES: [RegExp, WireRoute][] = [
  [/\/(?:shop-api\/v1\/credit-authorizations|credits\/credits\/authorize)\/batch$/, "batch"],
  [/\/(?:shop-api\/v1\/credit-authorizations|credits\/credits\/authorize)\/cancel$/, "cancel"],
  [/\/(?:shop-api\/v1\/credit-authorizations|credits\/credits\/authorize)\/submitted$/, "submitted"],
  [/\/(?:shop-api\/v1\/credit-authorizations|credits\/credits\/authorize)$/, "authorize"],
];

export function wireRoute(url: string): WireRoute | null {
  const path = new URL(url, "http://shop.invalid").pathname;
  return ROUTES.find(([re]) => re.test(path))?.[1] ?? null;
}

function equal(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function ttlSeconds(route: WireRoute): number {
  const setup = (wire.setup as Record<string, { ttlSeconds?: number } | undefined>)[route];
  return setup?.ttlSeconds ?? DEFAULT_TTL_SECONDS;
}

function resolve(value: unknown, ctx: { route: WireRoute; request: unknown; nowSeconds: number }, key = ""): unknown {
  if (Array.isArray(value)) return value.map((v) => resolve(v, ctx));
  if (!value || typeof value !== "object") return value;
  const v = value as Record<string, unknown>;
  if (v.$any === "string") return SALT;
  if (v.$any === "number") return key === "expiresAt" ? ctx.nowSeconds + ttlSeconds(ctx.route) : 0;
  if (typeof v.$length === "number") {
    const items = ((ctx.request as { items?: Record<string, unknown>[] } | null)?.items ?? []).slice(0, v.$length);
    return items.map(({ usdPriceCents, tradeId, contractAddress, itemId, name }) => ({
      usdCents: chargeForQuote(usdCents(Number(usdPriceCents))),
      ...(tradeId !== undefined ? { tradeId } : {}),
      ...(contractAddress !== undefined ? { contractAddress } : {}),
      ...(itemId !== undefined ? { itemId } : {}),
      ...(name !== undefined ? { name } : {}),
    }));
  }
  return Object.fromEntries(Object.entries(v).map(([k, inner]) => [k, resolve(inner, ctx, k)]));
}

export type FixtureTransportOptions = {
  /** The address signed requests authenticate as; null answers like an unsigned request. */
  auth?: string | null;
  /** Pin a case by name for a route, for the cases that differ only by server state. */
  scenario?: Partial<Record<WireRoute, string>>;
  now?: () => number;
  /** Every request the transport saw. */
  log?: TransportRequest[];
};

export function matchWireCase(route: WireRoute, req: TransportRequest, opts: FixtureTransportOptions = {}): WireCase | null {
  const auth = req.signed ? (opts.auth === undefined ? FIXTURE_BUYER : opts.auth) : null;
  const candidates = WIRE_CASES.filter((c) => c.route === route && c.auth === auth).filter((c) => {
    if (c.request.rawBody !== undefined) return req.body === c.request.rawBody;
    let body: unknown = null;
    try {
      body = req.body === undefined ? null : JSON.parse(req.body);
    } catch {
      return false;
    }
    return equal(body, c.request.body);
  });
  const pinned = opts.scenario?.[route];
  if (pinned) return candidates.find((c) => c.name === pinned) ?? null;
  const ok = (c: WireCase) => c.status >= 200 && c.status < 300;
  return candidates.find((c) => ok(c) && c.bodyAsserted) ?? candidates.find(ok) ?? candidates[0] ?? null;
}

export function fixtureTransport(opts: FixtureTransportOptions = {}): ShopTransport {
  const now = opts.now ?? Date.now;
  return async (req) => {
    opts.log?.push(req);
    const route = wireRoute(req.url);
    const found = route ? matchWireCase(route, req, opts) : null;
    if (!route || !found) return { status: 501, body: JSON.stringify({ error: "no pinned wire case answers this request" }) };
    let request: unknown = null;
    try {
      request = req.body ? JSON.parse(req.body) : null;
    } catch {
      request = null;
    }
    const body = found.body === null ? "" : JSON.stringify(resolve(found.body, { route, request, nowSeconds: Math.floor(now() / 1000) }));
    return { status: found.status, body };
  };
}

const WIRE_ENDPOINTS = [
  ENDPOINTS.authorizeCredit,
  ENDPOINTS.authorizeCreditBatch,
  ENDPOINTS.cancelCreditAuthorizations,
  ENDPOINTS.reportCreditSubmission,
];

/**
 * The same answers as MSW handlers, for stories that call the default client: both the canonical
 * path and the legacy alias of each credit-authorization route. A request carrying auth-chain
 * headers counts as signed.
 */
export function wireHandlers(opts: Omit<FixtureTransportOptions, "log"> = {}) {
  const transport = fixtureTransport(opts);
  const paths = WIRE_ENDPOINTS.flatMap((e) => [`/shop-api${e.path}`, e.legacy!]);
  return paths.map((path) =>
    http.post(`*${path}`, async ({ request }) => {
      const body = await request.text();
      const answer = await transport({
        url: request.url,
        method: "POST",
        headers: {},
        body: body || undefined,
        signed: request.headers.has("x-identity-auth-chain-0"),
        signPath: new URL(request.url).pathname,
      });
      return answer.body
        ? new HttpResponse(answer.body, { status: answer.status, headers: { "content-type": "application/json" } })
        : new HttpResponse(null, { status: answer.status });
    }),
  );
}
