import "@dcl/one-ui/foundations/foundations.css";
import { useState } from "react";
import { useSearchParams, type ShouldRevalidateFunctionArgs } from "react-router";
import { dehydrate, HydrationBoundary, QueryClient } from "@tanstack/react-query";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { siteUrl } from "@/data/lib/catalyst/bases";
import {
  calendarQuery,
  eventDetailQuery,
  liveNowQuery,
  placeDetailQuery,
  readEventId,
  readPlaceTarget,
  upcomingQuery,
} from "@/data/lib/catalyst/places/events-page";
import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { detectPlatform } from "@/features/download/options";
import { EventsView } from "@/features/events/EventsView";
import { TAB_PARAM } from "@/features/events/view";
import EventManagement from "@/features/explorer/event-management/EventManagement";

import type { Route } from "./+types/events";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = { agentMarkdown: "whatsOn" } satisfies AgentMarkdownHandle;

const FALLBACK: PageDefaults = { variant: "live_feed" };

export function meta() {
  return [{ title: "Events — Decentraland" }];
}

/**
 * The first document: the calendar is required (its outage answers 503); Live Now, Upcoming and an
 * event or place the URL opens are optional, and the browser fetches whatever failed here.
 */
export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const { sid, wrap } = await pageLoader(request, FALLBACK);
  const platform = detectPlatform(request.headers.get("user-agent"));
  const configured = siteUrl("/");
  const origin = /^https?:\/\//.test(configured) ? new URL(configured).origin : url.origin;
  const now = Date.now();
  if (url.searchParams.get(TAB_PARAM) === "my") {
    return wrap({ tab: "my" as const, sid, platform, origin, now, unavailable: false, dehydratedState: null });
  }
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const eventId = readEventId(url.searchParams);
  const place = eventId ? null : readPlaceTarget(url.searchParams);
  // A visitor who leaves stops the page's upstream reads: cancelling aborts each query's own signal.
  const stop = () => void client.cancelQueries();
  request.signal.addEventListener("abort", stop, { once: true });
  const [calendar] = await Promise.allSettled([
    client.fetchQuery(calendarQuery()),
    client.prefetchQuery(liveNowQuery()),
    client.prefetchQuery(upcomingQuery()),
    eventId ? client.prefetchQuery(eventDetailQuery(eventId)) : null,
    place ? client.prefetchQuery(placeDetailQuery(place)) : null,
  ]).finally(() => request.signal.removeEventListener("abort", stop));
  if (request.signal.aborted) {
    // Nobody is waiting for this page: that is not an outage, so it never answers 503.
    client.clear();
    throw request.signal.reason ?? new DOMException("The request was aborted.", "AbortError");
  }
  const unavailable = calendar.status === "rejected";
  const dehydratedState = dehydrate(client, { shouldDehydrateQuery: (query) => query.state.status === "success" });
  client.clear();
  return wrap(
    { tab: null, sid, platform, origin, now, unavailable, dehydratedState },
    { status: unavailable ? 503 : 200 },
  );
}

/** Client navigations read through the browser's query cache; the server loader runs for the first document only. */
export function clientLoader() {
  return null;
}

/** Opening a detail, paging days or switching tabs changes only the query; the page's reads are already cached. */
export function shouldRevalidate({ currentUrl, nextUrl, defaultShouldRevalidate }: ShouldRevalidateFunctionArgs) {
  return currentUrl.pathname === nextUrl.pathname ? false : defaultShouldRevalidate;
}

/**
 * Jump In's target depends on the visitor's device. The document carries the visitor's session id
 * (and may set its cookie), so no shared cache may store it.
 */
export function headers() {
  return { Vary: "User-Agent", "Cache-Control": "private, no-cache" };
}

export default function Events({ loaderData }: Route.ComponentProps) {
  const [params] = useSearchParams();
  // A client navigation has no loader data: the session and device come from the browser instead.
  const [first] = useState(() => ({
    sid: loaderData?.sid ?? "",
    platform: loaderData?.platform ?? detectPlatform(typeof navigator === "undefined" ? "" : navigator.userAgent),
    origin: loaderData?.origin ?? (typeof window === "undefined" ? "" : window.location.origin),
    initial: loaderData ? { now: loaderData.now, unavailable: loaderData.unavailable } : null,
  }));
  if (params.get(TAB_PARAM) === "my") return <EventManagement mode="mine" />;
  return (
    <HydrationBoundary state={loaderData?.dehydratedState ?? undefined}>
      <SitesChrome active="whatson">
        <EventsView sid={first.sid} platform={first.platform} origin={first.origin} initial={first.initial} />
      </SitesChrome>
    </HydrationBoundary>
  );
}
