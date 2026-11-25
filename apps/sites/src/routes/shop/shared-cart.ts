// The /shop/cart document for a shared cache (a CDN), behind SHOP_CART_SHARED_CACHE=true (off when unset) and
// PUBLIC_CATALYST_URL.
// The cart's server render holds no cart: the lines, the session and the wallet are in the browser, and the
// page reads them after hydration through its own private requests. With the setting on, a top-level
// navigation gets one visitor-free document that a shared cache may keep: root data without the session id,
// wallet or language, page data without the session id, and no session cookie.

import type { MiddlewareFunction } from "react-router";

import { markPublicResponse } from "@/core/lib/telemetry/session";
import { browserCatalystBase, processEnv, type BaseEnv } from "@/data/lib/catalyst/bases";

/**
 * A shared cache keeps the document for 60 s, so a flag change reaches every visitor soon. A browser keeps none: it
 * asks again on each visit, so the purge on a deploy reaches it at once and no old document names old build files.
 */
export const SHARED_CART_CACHE_CONTROL = "public, max-age=0, s-maxage=60";

/** The same 60 s for a CDN only (RFC 9213), which also serves a stale copy while it gets a new one. */
export const SHARED_CART_CDN_CACHE_CONTROL = "max-age=60, stale-while-revalidate=300";

/**
 * What selects the shared document: a frame gets the private one, with COEP when it is same-origin
 * (lib/frame-isolation.ts). The cart has no Markdown alternate, so `Accept` changes nothing.
 */
export const SHARED_CART_VARY = "Sec-Fetch-Dest";

/**
 * The document's own address must not come from a request's headers (root's `publicPageBase`, the page's `publicPageUrl`):
 * one request would then set it for every visitor. Thus the setting needs PUBLIC_CATALYST_URL.
 */
export function sharedCartEnabled(env: BaseEnv = processEnv()): boolean {
  return env.SHOP_CART_SHARED_CACHE?.trim() === "true" && browserCatalystBase(env) !== "";
}

/**
 * Only a browser's top-level navigation gets the shared document. The `.data` reads, frames and requests from
 * clients without fetch metadata keep the private document.
 */
function sharesDocument(request: Request): boolean {
  if (request.method !== "GET") return false;
  if (new URL(request.url).pathname.endsWith(".data")) return false;
  return request.headers.get("sec-fetch-dest") === "document";
}

export const sharedCartMiddleware: MiddlewareFunction<Response> = async ({ request, context }, next) => {
  if (!sharedCartEnabled() || !sharesDocument(request)) return next();
  markPublicResponse(context);
  const response = await next();
  // An error or a redirect keeps the route's private headers.
  if (response.status !== 200 || !response.headers.get("Content-Type")?.startsWith("text/html")) return response;
  response.headers.set("Cache-Control", SHARED_CART_CACHE_CONTROL);
  response.headers.set("CDN-Cache-Control", SHARED_CART_CDN_CACHE_CONTROL);
  response.headers.set("Vary", SHARED_CART_VARY);
  return response;
};
