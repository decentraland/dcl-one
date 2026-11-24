// The /shop/cart document for a shared cache (a CDN), behind SHOP_CART_SHARED_CACHE=true (off when unset).
// The cart's server render holds no cart: the lines, the session and the wallet are in the browser, and the
// page reads them after hydration through its own private requests. With the setting on, a top-level
// navigation gets one visitor-free document that a shared cache may keep: root data without the session id,
// wallet or language, page data without the session id, and no session cookie.

import type { MiddlewareFunction } from "react-router";

import { markPublicResponse } from "@/core/lib/telemetry/session";
import { processEnv, type BaseEnv } from "@/data/lib/catalyst/bases";

/** Short, so a deploy or a flag change reaches every visitor soon; a stale copy is served while one refreshes. */
export const SHARED_CART_CACHE_CONTROL = "public, max-age=60, stale-while-revalidate=300";

/**
 * What selects the shared document: a frame gets the private one, with COEP when it is same-origin
 * (lib/frame-isolation.ts). The cart has no Markdown alternate, so `Accept` changes nothing.
 */
export const SHARED_CART_VARY = "Sec-Fetch-Dest";

export function sharedCartEnabled(env: BaseEnv = processEnv()): boolean {
  return env.SHOP_CART_SHARED_CACHE?.trim() === "true";
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
  response.headers.set("Vary", SHARED_CART_VARY);
  return response;
};
