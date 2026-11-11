import { dehydrate, QueryClient } from "@tanstack/react-query";

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
import { TAB_PARAM } from "@/features/events/view";

/** The /events loader, outside the route module so its server-rendering tests run it in a browser environment too. */

const FALLBACK: PageDefaults = { variant: "live_feed" };

/**
 * The first document: the calendar is required (its outage answers 503); Live Now, Upcoming and an
 * event or place the URL opens are optional, and the browser fetches whatever failed here.
 */
export async function loadEventsPage(request: Request) {
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
