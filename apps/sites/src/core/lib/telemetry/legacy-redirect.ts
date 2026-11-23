import { useEffect } from "react";
import { redirect, useLocation } from "react-router";

import { useAnalyticsSid, useAnalyticsUserId } from "./page-view";
import { track, whenLegacyAnalyticsReady, withoutSensitiveParams } from "./track";
import { trackedUserDropped } from "./tracked-user";

// The legacy site's `Legacy Events Redirected` and `Legacy Places Redirected` (decentraland/sites
// `useLegacyRedirectTracking`): one per visit to a renamed section's old URL, sent from the visitor's
// browser once analytics was ready. Here the old URL answers with a server redirect, so it leaves its
// details in a short-lived cookie and the page it lands on sends the event, through the same gates
// (bot, exempt session) and with the same identity as that page's own page view.

export type LegacyRedirectOrigin = "events" | "places";

export const LEGACY_REDIRECT_COOKIE = "dcl_legacy_redirect";
/** Long enough for the destination page to load, short enough that a stale one is not sent later. */
const COOKIE_MAX_AGE_S = 60;
/** Browsers cap a cookie near 4 KB; an old URL whose details do not fit sends no event. */
const MAX_COOKIE_VALUE = 3000;

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


/** The `Set-Cookie` value that hands a redirect to the page it lands on; null when it would not fit. */
export function legacyRedirectCookie(request: Request, origin: LegacyRedirectOrigin, destination: string): string | null {
  const value = encodeURIComponent(JSON.stringify(legacyRedirectDetails(request, origin, destination)));
  if (value.length > MAX_COOKIE_VALUE) return null;
  return `${LEGACY_REDIRECT_COOKIE}=${value}; Path=/; Max-Age=${COOKIE_MAX_AGE_S}; SameSite=Lax; Secure`;
}

/** A renamed section's old URL: a redirect to `destination` carrying the legacy event's details. */
export function legacyRedirect(request: Request, origin: LegacyRedirectOrigin, destination: string): Response {
  const cookie = legacyRedirectCookie(request, origin, destination);
  return redirect(destination, cookie ? { headers: [["Set-Cookie", cookie]] } : undefined);
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

/** Reads and clears the handed-over redirect, so it is sent once whichever page reads it first. */
export function takeLegacyRedirect(): LegacyRedirect | null {
  if (typeof document === "undefined") return null;
  let raw: string | undefined;
  try {
    for (const part of document.cookie.split(";")) {
      const eq = part.indexOf("=");
      if (eq > 0 && part.slice(0, eq).trim() === LEGACY_REDIRECT_COOKIE) raw = part.slice(eq + 1).trim();
    }
    if (raw === undefined) return null;
    document.cookie = `${LEGACY_REDIRECT_COOKIE}=; Path=/; Max-Age=0; SameSite=Lax`;
  } catch {
    return null;
  }
  return parse(raw);
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
 * Sends the legacy redirect event a server redirect handed to this page, once the document's analytics
 * is ready (its first Sites page view left): a bot or an exempt session never sends it, as on the
 * legacy site. Checked on every navigation, since a client navigation can follow a redirect too.
 */
export function useLegacyRedirectEvent(): void {
  const { key } = useLocation();
  const sid = useAnalyticsSid();
  const userId = useAnalyticsUserId();
  useEffect(() => {
    if (!sid) return;
    const redirected = takeLegacyRedirect();
    if (redirected) whenLegacyAnalyticsReady(() => sendLegacyRedirect(redirected, sid, userId));
  }, [key, sid, userId]);
}
