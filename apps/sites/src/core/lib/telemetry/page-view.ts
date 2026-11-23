import { useEffect, useRef, useState } from "react";
import { useLocation, useRouteLoaderData } from "react-router";

import { adoptSitesSegmentConfig, page } from "./track";
import { trackedUserDropped } from "./tracked-user";

/** A Segment page call's name and properties, as the legacy site's `usePageViewTracking` took them. */
export type PageView = { name: string; properties?: Record<string, unknown> };

/**
 * The paths whose page view the page itself owns (blog, cast, social, places, storage), or that only
 * redirect (`/whats-on`, `/discover`, `/events/event`): the root's `page(pathname)` skips them. Ported
 * verbatim from the legacy site's `isPageTrackingExempt`, so `/events` itself is tracked.
 */
export function isPageTrackingExempt(path: string): boolean {
  // React Router matches routes case-insensitively, so `/Blog/x/y` is the blog post's page.
  const pathname = path.toLowerCase();
  if (pathname === "/blog" || pathname.startsWith("/blog/")) return true;
  if (pathname === "/whats-on" || pathname.startsWith("/whats-on/")) return true;
  if (pathname === "/discover" || pathname.startsWith("/discover/")) return true;
  if (pathname === "/events/event") return true;
  if (pathname === "/cast" || pathname.startsWith("/cast/")) return true;
  if (pathname === "/storage" || pathname.startsWith("/storage/")) return true;
  if (pathname === "/social" || pathname.startsWith("/social/")) return true;
  if (pathname === "/places" || pathname.startsWith("/places/")) return true;
  return false;
}

/**
 * Surfaces the legacy site never served, so its Segment source never counted them: the Shop and
 * Marketplace (the Shop reports its own views to its own source), the Builder, the Explorer, the docs
 * (docs.decentraland.org was its own site, and its responses here carry no session) and auth or
 * internal endpoints. Their pages send no Sites page view. A page new to this site outside these
 * (`/communities/new`) follows the legacy Layout's rule instead: `page(pathname)`.
 */
const OUTSIDE_LEGACY_SITES = [
  "/shop",
  "/shop-native",
  "/marketplace",
  "/marketplace-native",
  "/builder",
  "/play",
  "/explorer",
  "/explorer-map",
  "/client",
  "/creator-hub",
  "/editor-scene",
  "/docs",
  "/auth",
  "/connect",
  "/internal",
  "/api",
];

export function isOutsideLegacySites(path: string): boolean {
  const pathname = path.toLowerCase();
  return OUTSIDE_LEGACY_SITES.some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));
}

type RootSession = { sid?: string | null; analyticsUserId?: string | null };

/** The root loader's session id and Segment `userId` (the server-verified wallet only). */
function useRootSession(): { sid: string; userId: string } {
  const root = useRouteLoaderData("root") as RootSession | undefined;
  return { sid: root?.sid ?? "", userId: root?.analyticsUserId ?? "" };
}

/**
 * Sends `page(name, properties)` for `view` once per pathname, name and properties. A null view sends
 * nothing and forgets the last one, so leaving to an untracked page and coming back counts again.
 * A sign-out in progress leaves the account out at the moment of sending, whichever session (root's
 * or a fetched one) supplied it.
 */
function useSendPageView(view: PageView | null | undefined, sid: string, userId: string): void {
  const { pathname } = useLocation();
  // The key only compares views; what is sent is the view itself, so an undefined property survives.
  const key = view?.name ? JSON.stringify([pathname, view.name, view.properties ?? {}]) : null;
  const latest = useRef(view);
  latest.current = view;
  const sent = useRef<string | null>(null);
  useEffect(() => {
    const current = latest.current;
    if (!key || !current) {
      sent.current = null;
      return;
    }
    if (!sid || sent.current === key) return;
    sent.current = key;
    const user = trackedUserDropped() ? "" : userId;
    page(current.name, current.properties ?? {}, { sid, ...(user ? { userId: user } : {}) });
  }, [key, sid, userId]);
}

/**
 * Sends `page(name, properties)` once the view is known (a null view waits, as a page whose name is
 * its data does until the data is there) and again only when the pathname, the name or the
 * properties change: a search-only navigation, a re-render or StrictMode's second effect sends
 * nothing. The session id and the verified wallet come from the root loader; without a session id
 * nothing is sent.
 */
export function usePageView(view: PageView | null | undefined): void {
  const { sid, userId } = useRootSession();
  useSendPageView(view, sid, userId);
}

/** The root's page view: `page(pathname)`, except under `isPageTrackingExempt` and `isOutsideLegacySites`. */
export function useRoutePageView(): void {
  const { pathname } = useLocation();
  usePageView(isPageTrackingExempt(pathname) || isOutsideLegacySites(pathname) ? null : { name: pathname });
}

export const SESSION_ENDPOINT = "/internal/session";

type FetchedSession = { sid: string; userId: string };

const text = (value: unknown): string => (typeof value === "string" ? value : "");

/**
 * The visitor's session from `/internal/session`, null when it cannot be read. Its Segment transport
 * fills what the document lacks, before anyone sends with it.
 */
async function fetchSession(): Promise<FetchedSession | null> {
  try {
    const response = await fetch(SESSION_ENDPOINT, { credentials: "same-origin", cache: "no-store" });
    if (!response.ok) {
      void response.body?.cancel().catch(() => {});
      return null;
    }
    const body = (await response.json()) as Record<string, unknown> | null;
    const sid = text(body?.sid);
    if (!sid) return null;
    adoptSitesSegmentConfig({ segmentWriteKey: text(body?.segmentWriteKey), segmentApiHost: text(body?.segmentApiHost) });
    return { sid, userId: text(body?.userId) };
  } catch {
    return null;
  }
}

/**
 * One lookup at a time for the document: a first visit without a cookie would otherwise mint one sid
 * per request (StrictMode's replayed effect, two consumers), and a response cannot be unsent once its
 * cookie is set. Cleared once settled, so a later lookup reads the wallet afresh.
 */
let sessionLookup: Promise<FetchedSession | null> | null = null;

export function lookupSession(): Promise<FetchedSession | null> {
  sessionLookup ??= fetchSession().finally(() => {
    sessionLookup = null;
  });
  return sessionLookup;
}

/**
 * The 404 page's own view, `page('Not Found', { path })`, outside the paths whose pages own theirs and
 * the surfaces the legacy site never served. A URL no route matches renders without the root loader,
 * so its session is read from `/internal/session` instead.
 */
export function useNotFoundPageView(notFound: boolean): void {
  const { pathname } = useLocation();
  const root = useRootSession();
  const tracked = notFound && !isPageTrackingExempt(pathname) && !isOutsideLegacySites(pathname);
  const needsSession = tracked && !root.sid;
  const [fetched, setFetched] = useState<FetchedSession | null>(null);
  useEffect(() => {
    if (!needsSession) return;
    let live = true;
    void lookupSession().then((session) => {
      if (live && session) setFetched(session);
    });
    return () => {
      live = false;
    };
  }, [needsSession]);
  const session = root.sid ? root : (fetched ?? { sid: "", userId: "" });
  useSendPageView(tracked ? { name: "Not Found", properties: { path: pathname } } : null, session.sid, session.userId);
}
