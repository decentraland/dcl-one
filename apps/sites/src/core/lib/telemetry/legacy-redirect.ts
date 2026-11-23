import { useEffect, useLayoutEffect } from "react";
import { redirect, useLocation } from "react-router";

import { useAnalyticsSid, useAnalyticsUserId } from "./page-view";
import { track, whenLegacyAnalyticsReady, withoutSensitiveParams } from "./track";
import { trackedUserDropped } from "./tracked-user";

// The legacy site's `Legacy Events Redirected` and `Legacy Places Redirected` (decentraland/sites
// `useLegacyRedirectTracking`): one per visit to a renamed section's old URL, sent from the visitor's
// browser once analytics was ready, and dropped when it was not ready within 800 ms or the visitor left
// first. Here the old URL answers with a server redirect, so it hands its details to the page it lands on:
// a short-lived cookie named by a one-time id, and that id in the redirect's fragment. Only the navigation
// that followed this redirect carries the id, so neither another tab's redirect nor a direct visit to the
// same URL can take the hand-over. The landing page removes the fragment before its page view and sends
// the event through the same gates (bot, exempt session) and with the same identity as that page view.

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

/** The fragment that names a redirect's hand-over: `#dcl-legacy-redirect=<id>`. */
export const LEGACY_REDIRECT_FRAGMENT = "dcl-legacy-redirect";
const HANDOFF_ID = /^[0-9a-f]{16}$/;

type Handover = { location: string; cookie: string };

/**
 * The redirect's `Location` (the destination plus the hand-over's fragment) and its `Set-Cookie`; null when
 * the details would not fit in a cookie.
 */
export function legacyRedirectHandover(request: Request, origin: LegacyRedirectOrigin, destination: string): Handover | null {
  const value = encodeURIComponent(JSON.stringify(legacyRedirectDetails(request, origin, destination)));
  if (value.length > MAX_COOKIE_VALUE) return null;
  const id = handoffId();
  return {
    location: `${destination}#${LEGACY_REDIRECT_FRAGMENT}=${id}`,
    cookie: `${LEGACY_REDIRECT_COOKIE}${id}=${value}; Path=${pathOf(destination)}; Max-Age=${COOKIE_MAX_AGE_S}; SameSite=Lax; Secure`,
  };
}

/**
 * A renamed section's old URL: a redirect to `destination` carrying the legacy event's details. A response
 * that carries them is the visitor's own, so no cache stores it.
 */
export function legacyRedirect(request: Request, origin: LegacyRedirectOrigin, destination: string): Response {
  const handover = legacyRedirectHandover(request, origin, destination);
  if (!handover) return redirect(destination);
  return redirect(handover.location, {
    headers: [["Set-Cookie", handover.cookie], ["Cache-Control", "private, no-store"]],
  });
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

/** The hand-over id a URL fragment names, or null. */
export function handoverId(hash: string): string | null {
  const prefix = `#${LEGACY_REDIRECT_FRAGMENT}=`;
  if (!hash.startsWith(prefix)) return null;
  const id = hash.slice(prefix.length);
  return HANDOFF_ID.test(id) ? id : null;
}

/**
 * Reads and clears the cookie of hand-over `id`, so it is sent once. It is returned only when it was handed
 * to the URL being shown (`pathname` + `search`); one that cannot be read sends nothing.
 */
export function takeLegacyRedirect(id: string, pathname: string, search: string): LegacyRedirect | null {
  if (typeof document === "undefined") return null;
  const name = `${LEGACY_REDIRECT_COOKIE}${id}`;
  try {
    let raw: string | undefined;
    for (const part of document.cookie.split(";")) {
      const eq = part.indexOf("=");
      if (eq > 0 && part.slice(0, eq).trim() === name) raw = part.slice(eq + 1).trim();
    }
    if (raw === undefined) return null;
    for (const path of new Set([pathOf(pathname), "/"])) document.cookie = `${name}=; Path=${path}; Max-Age=0; SameSite=Lax`;
    const parsed = parse(raw);
    return parsed && parsed.destination === withoutSensitiveParams(`${pathname}${search}`) ? parsed : null;
  } catch {
    return null;
  }
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
 * A hand-over taken by a navigation: its deadline is fixed when it lands, and once it is sent, expired or
 * left behind it never goes again, whatever re-runs (a session that arrives later, a returning history entry).
 */
type Landing = {
  key: string;
  redirect: LegacyRedirect | null;
  deadline: number;
  state: "pending" | "sent" | "expired" | "navigated" | "invalid";
};

/** The hand-overs this document's navigations took, by the navigation's location key. */
const landings = new Map<string, Landing>();

// A server render takes nothing; in the browser the hand-over is taken before any page's passive effects run,
// so no page view sees the fragment.
const useBrowserLayoutEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;

function removeFragment(pathname: string, search: string): void {
  try {
    window.history.replaceState(window.history.state, "", `${pathname}${search}`);
  } catch {
    // The URL keeps its fragment; the event is unaffected.
  }
}

/**
 * Sends the legacy redirect event the redirect this navigation followed handed over, once the document's
 * analytics is ready (its first Sites page view left), as the legacy hook did: a bot or an exempt session
 * never sends it, nor does a page whose analytics is still not ready 800 ms after landing, nor one the visitor
 * leaves first. Checked on every navigation, since a client navigation can follow a redirect too. Call it
 * before the root's page view.
 */
export function useLegacyRedirectEvent(): void {
  const { key, pathname, search, hash } = useLocation();
  const sid = useAnalyticsSid();
  const userId = useAnalyticsUserId();

  useBrowserLayoutEffect(() => {
    for (const landing of landings.values()) {
      if (landing.state === "pending" && landing.key !== key) landing.state = "navigated";
    }
    if (landings.has(key)) return;
    const id = handoverId(hash);
    if (!id) return;
    removeFragment(pathname, search);
    const redirect = takeLegacyRedirect(id, pathname, search);
    landings.set(key, {
      key,
      redirect,
      deadline: Date.now() + LEGACY_REDIRECT_TIMEOUT_MS,
      state: redirect ? "pending" : "invalid",
    });
  }, [key, pathname, search, hash]);

  useEffect(() => {
    const landing = landings.get(key);
    if (!landing || landing.state !== "pending" || !landing.redirect || !sid) return;
    const remaining = landing.deadline - Date.now();
    if (remaining <= 0) {
      landing.state = "expired";
      return;
    }
    const redirected = landing.redirect;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const cancel = whenLegacyAnalyticsReady(() => {
      clearTimeout(timer);
      if (landing.state !== "pending") return;
      landing.state = "sent";
      sendLegacyRedirect(redirected, sid, userId);
    });
    if (landing.state !== "pending") return;
    timer = setTimeout(() => {
      cancel();
      if (landing.state === "pending") landing.state = "expired";
    }, remaining);
    return () => {
      cancel();
      clearTimeout(timer);
    };
  }, [key, sid, userId]);
}
