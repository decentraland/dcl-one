import "@dcl/one-ui/foundations/foundations.css";
import { useState } from "react";
import { type ShouldRevalidateFunctionArgs } from "react-router";
import { HydrationBoundary } from "@tanstack/react-query";
import SitesLayout from "@/features/components/chrome/SitesLayout";
import type { SitesLocaleHandle } from "@dcl/one-ui";

import { detectPlatform } from "@/features/download/options";
import { EventsLocaleProvider, useEventsFooterCopy } from "@/features/events/locale";
import { EventsView } from "@/features/events/EventsView";
import { EventAdminTabs } from "@/features/explorer/event-management/EventAdminTabs";
import type { JumpPlaceRecord } from "@/data/lib/catalyst/jump/jump";
import type { Event } from "@/data/lib/catalyst/places/events";
import {
  dehydratedData,
  eventsPageKeys,
  readEventId,
  readPlaceTarget,
  type LiveNowData,
} from "@/data/lib/catalyst/places/events-page";
import {
  EVENTS_SECTION,
  destinationMeta,
  eventCanonicalPath,
  eventCard,
  eventJsonLd,
  eventsListingJsonLd,
  parcelCard,
  sectionCard,
  worldCard,
} from "../lib/destination-meta";
import { loadEventsPage } from "../lib/events-loader";
import { absoluteUrl, publicBase } from "../lib/social-meta";

import type { Route } from "./+types/events";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = {
  agentMarkdown: "whatsOn",
  sitesLocalized: true,
} satisfies AgentMarkdownHandle & SitesLocaleHandle;

/**
 * The card of the event (`?id=`), World (`?world=`) or parcel (`?position=`) the link opens, from the
 * reads the loader already made; the section card with its live and upcoming lists otherwise.
 */
export function meta({ loaderData, location, matches }: Route.MetaArgs) {
  const base = publicBase(matches);
  const params = new URLSearchParams(location.search);
  const state = loaderData?.dehydratedState;
  const eventId = readEventId(params);
  const event = eventId ? dehydratedData<Event | null>(state, eventsPageKeys.detail(eventId)) : null;
  if (event) {
    const url = absoluteUrl(base, eventCanonicalPath(event.id));
    const jsonLd = url ? eventJsonLd(event, url) : null;
    return destinationMeta(eventCard(event), { url, jsonLd: jsonLd ? [jsonLd] : undefined });
  }
  const place = eventId ? null : readPlaceTarget(params);
  if (place) {
    const row = dehydratedData<JumpPlaceRecord | null>(state, eventsPageKeys.place(place));
    return "world" in place
      ? destinationMeta(worldCard(place.world, row), { url: absoluteUrl(base, `/events?world=${encodeURIComponent(place.world)}`) })
      : destinationMeta(parcelCard(place.position.join(","), row), { url: absoluteUrl(base, `/events?position=${place.position.join(",")}`) });
  }
  const live = dehydratedData<LiveNowData>(state, eventsPageKeys.live());
  const upcoming = dehydratedData<Event[]>(state, eventsPageKeys.upcoming());
  const listing =
    base && live && upcoming && !eventId
      ? eventsListingJsonLd(base, [
          { name: "Live now", events: live.events },
          { name: "Upcoming in the next 24 hours", events: upcoming },
        ])
      : null;
  return destinationMeta(sectionCard(EVENTS_SECTION), { url: absoluteUrl(base, "/events"), jsonLd: listing ? [listing] : undefined });
}

/**
 * The first document: the calendar is required (its outage answers 503); Live Now, Upcoming and an
 * event or place the URL opens are optional, and the browser fetches whatever failed here.
 */
export function loader({ request, context }: Route.LoaderArgs) {
  return loadEventsPage(request, context);
}

/** Client navigations read through the browser's query cache; the server loader runs for the first document only. */
export function clientLoader() {
  return null;
}

/** Opening a detail, paging days or switching tabs changes only the query; the page's reads are already cached. */
export function shouldRevalidate({
  currentUrl,
  nextUrl,
  defaultShouldRevalidate,
}: ShouldRevalidateFunctionArgs) {
  return currentUrl.pathname === nextUrl.pathname
    ? false
    : defaultShouldRevalidate;
}

/**
 * Jump In's target depends on the visitor's device. The document carries the visitor's session id
 * (and may set its cookie), so no shared cache may store it.
 */
export function headers() {
  return { Vary: "User-Agent", "Cache-Control": "private, no-cache" };
}

export default function Events(props: Route.ComponentProps) {
  return <EventsLocaleProvider><EventsRoute {...props} /></EventsLocaleProvider>;
}

function EventsRoute({ loaderData }: Route.ComponentProps) {
  const footerCopy = useEventsFooterCopy();
  // A client navigation has no loader data: the session and device come from the browser instead.
  const [first] = useState(() => ({
    sid: loaderData?.sid ?? "",
    platform:
      loaderData?.platform ??
      detectPlatform(
        typeof navigator === "undefined" ? "" : navigator.userAgent,
      ),
    origin:
      loaderData?.origin ??
      (typeof window === "undefined" ? "" : window.location.origin),
    pageOrigin:
      loaderData?.pageOrigin ??
      (typeof window === "undefined" ? "" : window.location.origin),
    initial: loaderData
      ? { now: loaderData.now, unavailable: loaderData.unavailable }
      : null,
  }));
  // My Events (`?tab=my`) is a view of this page: EventsView swaps the All Events section for it.
  return (
    <HydrationBoundary state={loaderData?.dehydratedState ?? undefined}>
      <SitesLayout active="whatson" headerSurface="overlay" overlayNav footerCopy={footerCopy}>
        <EventAdminTabs active="events" />
        <EventsView
          sid={first.sid}
          platform={first.platform}
          origin={first.origin}
          pageOrigin={first.pageOrigin}
          initial={first.initial}
        />
      </SitesLayout>
    </HydrationBoundary>
  );
}
