import "@dcl/one-ui/foundations/foundations.css";
import { useState } from "react";
import {
  useSearchParams,
  type ShouldRevalidateFunctionArgs,
} from "react-router";
import { HydrationBoundary } from "@tanstack/react-query";
import SitesChrome from "@ui/web/frames/SitesChrome";
import type { SitesLocaleHandle } from "@ui/web/frames/sites-locale";

import { detectPlatform } from "@/features/download/options";
import { EventsLocaleProvider, useEventsFooterCopy } from "@/features/events/locale";
import { EventsView } from "@/features/events/EventsView";
import { TAB_PARAM } from "@/features/events/view";
import EventManagement from "@/features/explorer/event-management/EventManagement";
import { loadEventsPage } from "../lib/events-loader";

import type { Route } from "./+types/events";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";

export const handle = {
  agentMarkdown: "whatsOn",
  sitesLocalized: true,
} satisfies AgentMarkdownHandle & SitesLocaleHandle;

export function meta() {
  return [{ title: "Events — Decentraland" }];
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
  const [params] = useSearchParams();
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
    initial: loaderData
      ? { now: loaderData.now, unavailable: loaderData.unavailable }
      : null,
  }));
  return params.get(TAB_PARAM) === "my" ? <EventManagement mode="mine" /> : (
    <HydrationBoundary state={loaderData?.dehydratedState ?? undefined}>
      <SitesChrome active="whatson" headerSurface="overlay" overlayNav footerCopy={footerCopy}>
        <EventsView
          sid={first.sid}
          platform={first.platform}
          origin={first.origin}
          initial={first.initial}
        />
      </SitesChrome>
    </HydrationBoundary>
  );
}
