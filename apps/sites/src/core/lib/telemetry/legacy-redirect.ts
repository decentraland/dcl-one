import { useEffect } from "react";
import { redirect, useLocation } from "react-router";

import { useAnalyticsSid, useAnalyticsUserId } from "./page-view";
import { track, whenLegacyAnalyticsReady, withoutSensitiveParams } from "./track";
import { trackedUserDropped } from "./tracked-user";

// The legacy site's `Legacy Events Redirected` and `Legacy Places Redirected` (decentraland/sites
// `useLegacyRedirectTracking`): one per visit to a renamed section's old URL, sent from the visitor's
// browser once analytics was ready, and dropped when it was not ready within 800 ms or the visitor left
// first. Here the old URL answers with a server redirect, so it hands its details to the page it lands on
// in a short-lived cookie of its own (one per redirect, scoped to the destination's path, read only on
// the exact destination URL), and that page sends the event through the same gates (bot, exempt
// session) and with the same identity as its own page view.

export type LegacyRedirectOrigin = "events" | "places";

/** Each redirect's cookie is this prefix plus its own id, so concurrent redirects never share one. */
export const LEGACY_REDIRECT_COOKIE = "dcl_legacy_redirect_";
/** Long enough for the destination page to load, short enough that a stale one is not sent later. */
const COOKIE_MAX_AGE_S = 60;
/** Browsers cap a cookie near 4 KB; an old URL whose details do not fit sends no event. */
const MAX_COOKIE_VALUE = 3000;
/** decentraland/sites `REDIRECT_TIMEOUT_MS`: analytics not ready this long after landing sends nothing. */
export const LEGACY_REDIRECT_TIMEOUT_MS = 800;

type LegacyRedirect = {
  origin: LegacyRedirectOrigin;
  source: string;
  destination: string;
  preservedParams: Record<string, string>;
};

/**
 * The legacy payload of a redirect: `source` is the old path and query, `destination` where the visitor
 * was sent, `preservedParams` the old URL's query as an object. Credentials are dropped from all three.
 */
export function legacyRedirectDetails(request: Request, origin: LegacyRedirectOrigin, destination: string): LegacyRedirect {
  const url = new URL(request.url);
  const source = withoutSensitiveParams(`${url.pathname}${url.search}`);
  const q = source.indexOf("?");
  const preservedParams = Object.fromEntries(new URLSearchParams(q < 0 ? "" : source.slice(q)));
  return { origin, source, destination: withoutSensitiveParams(destination), preservedParams };
}

function handoffId(): string {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * The destination's path, so the cookie is only sent to and visible on it; `/` when the path holds a
 * character a cookie attribute cannot carry (the exact-URL match still applies).
 */
function pathOf(destination: string): string {
  const end = destination.search(/[?#]/);
  const path = end < 0 ? destination : destination.slice(0, end);
  return /^\/[^;,\s]*$/.test(path) ? path : "/";
}

/** The `Set-Cookie` value that hands a redirect to the page it lands on; null when it would not fit. */
export function legacyRedirectCookie(request: Request, origin: LegacyRedirectOrigin, destination: string): string | null {
  const value = encodeURIComponent(JSON.stringify(legacyRedirectDetails(request, origin, destination)));
  if (value.length > MAX_COOKIE_VALUE) return null;
  const path = pathOf(destination);
  return `${LEGACY_REDIRECT_COOKIE}${handoffId()}=${value}; Path=${path}; Max-Age=${COOKIE_MAX_AGE_S}; SameSite=Lax; Secure`;
}

/**
 * A renamed section's old URL: a redirect to `destination` carrying the legacy event's details. A response
 * that carries them is the visitor's own, so no cache stores it.
 */
export function legacyRedirect(request: Request, origin: LegacyRedirectOrigin, destination: string): Response {
  const cookie = legacyRedirectCookie(request, origin, destination);
  if (!cookie) return redirect(destination);
  return redirect(destination, { headers: [["Set-Cookie", cookie], ["Cache-Control", "private, no-store"]] });
}

const isText = (value: unknown): value is string => typeof value === "string";

function parse(raw: string): LegacyRedirect | null {
  try {
    const value = JSON.parse(decodeURIComponent(raw)) as Partial<LegacyRedirect> | null;
    if (!value || (value.origin !== "events" && value.origin !== "places")) return null;
    if (!isText(value.source) || !isText(value.destination)) return null;
    const params = value.preservedParams;
    if (!params || typeof params !== "object" || Array.isArray(params)) return null;
    if (!Object.values(params).every(isText)) return null;
    return { origin: value.origin, source: value.source, destination: value.destination, preservedParams: params };
  } catch {
    return null;
  }
}

/**
 * Reads and clears the redirect handed to the URL being shown (`pathname` + `search`), so it is sent once
 * and only by the page it was meant for: another tab's redirect, or one to another URL, stays for its own
 * page. A hand-over that cannot be read is cleared and sends nothing.
 */
export function takeLegacyRedirect(pathname: string, search: string): LegacyRedirect | null {
  if (typeof document === "undefined") return null;
  const here = withoutSensitiveParams(`${pathname}${search}`);
  try {
    for (const part of document.cookie.split(";")) {
      const eq = part.indexOf("=");
      const name = eq > 0 ? part.slice(0, eq).trim() : "";
      if (!name.startsWith(LEGACY_REDIRECT_COOKIE)) continue;
      const parsed = parse(part.slice(eq + 1).trim());
      if (parsed && parsed.destination !== here) continue;
      for (const path of new Set([pathOf(pathname), "/"])) document.cookie = `${name}=; Path=${path}; Max-Age=0; SameSite=Lax`;
      if (parsed) return parsed;
    }
  } catch {
    return null;
  }
  return null;
}

/** The legacy key order: `source`, `destination`, `origin`, `preservedParams`, `timestamp` (ISO, at sending). */
type LegacyRedirectProps<O extends LegacyRedirectOrigin> = {
  source: string;
  destination: string;
  origin: O;
  preservedParams: Record<string, string>;
  timestamp: string;
};

function sendLegacyRedirect(redirected: LegacyRedirect, sid: string, userId: string): void {
  const user = userId && !trackedUserDropped() ? userId : undefined;
  const ctx = { sid, ...(user ? { userId: user } : {}), legacy: true };
  const props = <O extends LegacyRedirectOrigin>(origin: O): LegacyRedirectProps<O> => ({
    source: redirected.source,
    destination: redirected.destination,
    origin,
    preservedParams: redirected.preservedParams,
    timestamp: new Date().toISOString(),
  });
  if (redirected.origin === "events") track("Legacy Events Redirected", props("events"), ctx);
  else track("Legacy Places Redirected", props("places"), ctx);
}

/**
 * Sends the legacy redirect event a server redirect handed to this URL, once the document's analytics is
 * ready (its first Sites page view left), as the legacy hook did: a bot or an exempt session never sends
 * it, nor does a page whose analytics is still not ready 800 ms after landing, nor one the visitor leaves
 * first. Checked on every navigation, since a client navigation can follow a redirect too.
 */
export function useLegacyRedirectEvent(): void {
  const { key, pathname, search } = useLocation();
  const sid = useAnalyticsSid();
  const userId = useAnalyticsUserId();
  useEffect(() => {
    if (!sid) return;
    const redirected = takenFor(key, pathname, search);
    if (!redirected) return;
    let deadline: ReturnType<typeof setTimeout> | undefined;
    const cancel = whenLegacyAnalyticsReady(() => {
      clearTimeout(deadline);
      handled.add(key);
      sendLegacyRedirect(redirected, sid, userId);
    });
    if (handled.has(key)) return;
    deadline = setTimeout(cancel, LEGACY_REDIRECT_TIMEOUT_MS);
    return () => {
      cancel();
      clearTimeout(deadline);
    };
  }, [key, pathname, search, sid, userId]);
}

/**
 * The hand-over read for a navigation, kept per navigation so a re-run effect (StrictMode, a session that
 * arrives later) sees the same one instead of a cleared cookie; one sent is never sent again.
 */
const taken = new Map<string, LegacyRedirect | null>();
const handled = new Set<string>();

function takenFor(key: string, pathname: string, search: string): LegacyRedirect | null {
  if (handled.has(key)) return null;
  if (!taken.has(key)) taken.set(key, takeLegacyRedirect(pathname, search));
  return taken.get(key) ?? null;
}
