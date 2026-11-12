import { createContext, data, type MiddlewareFunction, type RouterContextProvider } from "react-router";

const SID_COOKIE = "sid";
const SID_MAX_AGE = 60 * 60 * 24 * 365;
const WALLET_COOKIE = "dcl_wallet_verified";
const WALLET_MAX_AGE = 60 * 60 * 24 * 30;
const ADDRESS = /^0x[0-9a-f]{40}$/;

export function parseCookies(header: string | null | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  if (!header) return out;
  for (const part of header.split(";")) {
    const eq = part.indexOf("=");
    if (eq === -1) continue;
    const key = part.slice(0, eq).trim();
    if (!key) continue;
    const value = part.slice(eq + 1).trim();
    try {
      out[key] = decodeURIComponent(value);
    } catch {
      out[key] = value;
    }
  }
  return out;
}

export function readSid(request: Request): string | null {
  const sid = parseCookies(request.headers.get("cookie"))[SID_COOKIE];
  return sid && sid.length > 0 ? sid : null;
}

function sidSharedParent(): string | undefined {
  const raw = typeof process !== "undefined" ? process.env?.SID_COOKIE_DOMAIN : undefined;
  const parent = raw?.trim().replace(/^\./, "").toLowerCase();
  return parent ? parent : undefined;
}

export function sharedSidDomain(request: Request): string | undefined {
  const parent = sidSharedParent();
  if (!parent) return undefined;
  let hostname: string;
  try {
    hostname = new URL(request.url).hostname;
  } catch {
    return undefined;
  }
  return hostname === parent || hostname.endsWith(`.${parent}`) ? parent : undefined;
}

export function hasSplitSidCookie(request: Request): boolean {
  const header = request.headers.get("cookie");
  if (!header) return false;
  let seen = 0;
  for (const part of header.split(";")) {
    const eq = part.indexOf("=");
    if (eq !== -1 && part.slice(0, eq).trim() === SID_COOKIE) seen += 1;
  }
  return seen > 1;
}

export function createSid(): string {
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return crypto.randomUUID();
    }
  } catch {
  }
  let sid = "";
  for (let i = 0; i < 32; i++) sid += Math.floor(Math.random() * 16).toString(16);
  return sid;
}

export function ensureSid(request: Request): { sid: string; created: boolean } {
  const existing = readSid(request);
  if (existing) return { sid: existing, created: false };
  return { sid: createSid(), created: true };
}

export function serializeSidCookie(
  sid: string,
  opts?: { domain?: string },
): string {
  return [
    `${SID_COOKIE}=${encodeURIComponent(sid)}`,
    "Path=/",
    `Max-Age=${SID_MAX_AGE}`,
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
    ...(opts?.domain ? [`Domain=${opts.domain}`] : []),
  ].join("; ");
}

export function expireSidCookie(opts?: { domain?: string }): string {
  return [
    `${SID_COOKIE}=`,
    "Path=/",
    "Max-Age=0",
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
    ...(opts?.domain ? [`Domain=${opts.domain}`] : []),
  ].join("; ");
}

export function readVerifiedWallet(request: Request): string | null {
  const wallet = parseCookies(request.headers.get("cookie"))[WALLET_COOKIE];
  return wallet && ADDRESS.test(wallet) ? wallet : null;
}

export function serializeVerifiedWalletCookie(address: string): string {
  return [
    `${WALLET_COOKIE}=${encodeURIComponent(address.toLowerCase())}`,
    "Path=/",
    `Max-Age=${WALLET_MAX_AGE}`,
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
  ].join("; ");
}

export function clearVerifiedWalletCookie(): string {
  return [
    `${WALLET_COOKIE}=`,
    "Path=/",
    "Max-Age=0",
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
  ].join("; ");
}

export type RequestSession = { sid: string; created: boolean };

/** The session of the request being served, set once by `sessionMiddleware` before any loader runs. */
export const requestSessionContext = createContext<RequestSession | null>(null);

/** The cookies a response carries for its session: a new sid, or moving a split cookie to the shared domain. */
export function sidCookieHeaders(request: Request, session: RequestSession): string[] {
  const domain = sharedSidDomain(request);
  if (session.created) return [serializeSidCookie(session.sid, { domain })];
  if (domain && hasSplitSidCookie(request)) return [serializeSidCookie(session.sid, { domain }), expireSidCookie()];
  return [];
}

function withCookies(response: Response, cookies: string[]): Response {
  if (cookies.length === 0) return response;
  try {
    for (const cookie of cookies) response.headers.append("Set-Cookie", cookie);
    return response;
  } catch {
    // A response with immutable headers (a fetched one, for example) is copied first.
    const copy = new Response(response.body, response);
    for (const cookie of cookies) copy.headers.append("Set-Cookie", cookie);
    return copy;
  }
}

/** Set by a route whose responses a shared cache may store (see `markPublicResponse`). */
export const publicResponseContext = createContext<boolean>(false);

/**
 * Called from the middleware of a route served with a public `Cache-Control` (the docs), before its
 * loaders run: such a response is stored once and served to everyone, so it must carry neither a
 * visitor's session id nor a session cookie.
 */
export function markPublicResponse(context: Readonly<RouterContextProvider>): void {
  context.set(publicResponseContext, true);
}

/** `public` or `s-maxage`: either lets a shared cache store the response. */
function isPublic(response: Response): boolean {
  return /(^|,)\s*(public|s-maxage)\b/i.test(response.headers.get("Cache-Control") ?? "");
}

/**
 * Root middleware: reads the sid cookie or mints one, once per request, before any loader, so every
 * loader of the request (root included) and the error page share it. The cookie goes on the
 * response once, here, whatever answered, a redirect or an error included, except on a response a
 * shared cache may store.
 */
export const sessionMiddleware: MiddlewareFunction<Response> = async ({ request, context }, next) => {
  const session = ensureSid(request);
  context.set(requestSessionContext, session);
  const response = await next();
  if (context.get(publicResponseContext) || isPublic(response)) return response;
  return withCookies(response, sidCookieHeaders(request, session));
};

/**
 * The request's session as the middleware set it. Without the middleware (a loader called on its
 * own, as in tests) it is read or minted here, and `ownsCookie` says the caller must set the cookie.
 */
export function requestSession(
  request: Request,
  context: Readonly<RouterContextProvider>,
): RequestSession & { ownsCookie: boolean } {
  const fromMiddleware = context.get(requestSessionContext);
  return fromMiddleware ? { ...fromMiddleware, ownsCookie: false } : { ...ensureSid(request), ownsCookie: true };
}

/**
 * The `Set-Cookie` headers a response built by hand (a redirect) must carry itself: only when no
 * middleware set the session, by the same rules (`sidCookieHeaders`) the middleware uses.
 */
export function ownedSessionCookies(
  request: Request,
  session: RequestSession & { ownsCookie: boolean },
): [string, string][] | undefined {
  const cookies = session.ownsCookie ? sidCookieHeaders(request, session) : [];
  return cookies.length > 0 ? cookies.map((cookie) => ["Set-Cookie", cookie]) : undefined;
}

/** The session id page data may carry: none on a response a shared cache may store. */
export function pageSid(request: Request, context: Readonly<RouterContextProvider>): string | null {
  return context.get(publicResponseContext) ? null : requestSession(request, context).sid;
}

/** A page loader's session. Under `markPublicResponse` it throws: shared page data must carry no sid. */
export function sidLoader(request: Request, context: Readonly<RouterContextProvider>) {
  if (context.get(publicResponseContext)) throw new Error("sidLoader called on a publicly cacheable response");
  const { sid, created, ownsCookie } = requestSession(request, context);
  const wallet = readVerifiedWallet(request);
  const cookies = ownsCookie ? sidCookieHeaders(request, { sid, created }) : [];
  const headers = cookies.length > 0 ? new Headers(cookies.map((cookie) => ["Set-Cookie", cookie])) : undefined;
  return {
    sid,
    wallet,
    userKey: wallet ?? sid,
    created,
    wrap: <T,>(payload: T, init?: { status?: number }) =>
      data(payload, { status: init?.status, headers }),
  };
}
