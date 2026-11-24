import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import { CommunityEventsList } from "@dcl/one-ui/apps/sites/social/CommunityLists";

import { useAuth } from "@/data/lib/auth/context";
import { eventDetailQuery, readEventId } from "@/data/lib/catalyst/places/events-page";
import { eventsQuery } from "@/data/lib/community/queries";
import { openSignIn } from "@/features/components/auth/signin-store";
import type { Zone } from "@/features/components/event-schedule";
import { detectPlatform } from "@/features/download/options";
import { EventDetail, eventJumpIn, isPlainClick, remindControl } from "@/features/events/EventDetail";
import { useCreatorProfiles } from "@/features/events/useCreatorProfiles";
import { REMIND_FAILED, useEventReminders } from "@/features/events/useEventReminders";
import { creatorWallets, eventHref, upcomingCardProps } from "@/features/events/view";
import { useLoadMore } from "@/features/social/community/useLoadMore";
import { ACTION_PARAM, NONCE_PARAM } from "@/features/social/community/view";

import { listMore, listStatus } from "./list-state";

/** The search parameter naming the event whose detail is open over the page. */
const EVENT_PARAM = "event";

/**
 * Marks the history entry a plain click pushed, so closing that detail goes back to the entry before.
 * The mark names this document: a reload keeps history state, but the entry before is then another document's.
 */
const OPENED_HERE = "communityEventOpened";
const THIS_DOCUMENT = Math.random().toString(36).slice(2);

const openedHere = (state: unknown) =>
  typeof state === "object" && state !== null && (state as Record<string, unknown>)[OPENED_HERE] === THIS_DOCUMENT;

/**
 * The server renders times in UTC with no calendar link (it has no origin); once hydrated the
 * browser re-renders them in the visitor's zone, from the visitor's clock, so both renders match.
 */
function useViewer(initialNow: number) {
  const [viewer, setViewer] = useState<{ zone: Zone; origin: string; now: number }>({
    zone: "UTC",
    origin: "",
    now: initialNow,
  });
  useEffect(() => {
    const read = () => setViewer({ zone: undefined, origin: window.location.origin, now: Date.now() });
    read();
    // "Starts in N mins" keeps counting down, as on the events page.
    const tick = setInterval(read, 60_000);
    return () => clearInterval(tick);
  }, []);
  return viewer;
}

/**
 * The community's upcoming events and the one whose detail is open (`?event=`), shared by the list
 * and the dialog the page renders outside its own stacking context. Disabled while the visitor may
 * not see the community's content: nothing is read and no detail opens.
 *
 * The open event is the listed one with that id. One the list has not loaded (a later page, or a
 * shared link) is read by id, as `/events?id=` does, and shown only if this community hosts it. An
 * id the catalogue does not know, or another community's event, drops the parameter; an outage keeps it.
 */
export function useCommunityEvents(
  communityId: string,
  { enabled, now: initialNow }: { enabled: boolean; now: number },
) {
  const { identity } = useAuth();
  const query = useInfiniteQuery({ ...eventsQuery(communityId), enabled });
  // Cached pages answer even while disabled; a hidden community shows, and reads profiles for, none.
  const events = useMemo(
    () => (enabled ? (query.data?.pages.flatMap((page) => page.events) ?? []) : []),
    [enabled, query.data],
  );
  const reminders = useEventReminders(enabled ? identity : null, () => openSignIn());
  const viewer = useViewer(initialNow);

  const location = useLocation();
  const navigate = useNavigate();
  const eventId = readEventId(new URLSearchParams(location.search), EVENT_PARAM);
  const listed = eventId ? events.find((event) => event.id === eventId) : undefined;
  // The first page usually holds it, so the read by id waits for that page to settle.
  const detail = useQuery({
    ...eventDetailQuery(eventId ?? ""),
    enabled: enabled && Boolean(eventId) && !listed && !query.isPending,
  });
  // Like the list, a cached read (another page's, or before a sign-out) counts for nothing while disabled.
  const hosted = enabled && detail.data && detail.data.community_id === communityId ? detail.data : null;
  const open = listed ?? hosted ?? null;

  const groups = useMemo(() => [creatorWallets(events), creatorWallets(hosted ? [hosted] : [])], [events, hosted]);
  const profiles = useCreatorProfiles(groups);

  const show = useCallback(
    (id: string) => {
      // A second click before the dialog covers the card would push a second entry for the same event.
      if (id === eventId) return;
      const next = new URLSearchParams(location.search);
      // A sign-in return's action stays on the entry it arrived on: it is deferred until the visitor comes
      // back to that entry (Close, or Back later), and runs only if that happens before the action expires.
      next.delete(ACTION_PARAM);
      next.delete(NONCE_PARAM);
      next.set(EVENT_PARAM, id);
      void navigate({ search: `?${next}` }, { preventScrollReset: true, state: { [OPENED_HERE]: THIS_DOCUMENT } });
    },
    [eventId, location.search, navigate],
  );
  const close = useCallback(() => {
    if (openedHere(location.state)) {
      void navigate(-1);
      return;
    }
    const next = new URLSearchParams(location.search);
    next.delete(EVENT_PARAM);
    const search = next.toString();
    void navigate({ search: search ? `?${search}` : "" }, { replace: true, preventScrollReset: true });
  }, [location.search, location.state, navigate]);

  // A community the visitor may not see never rewrites the URL.
  const missing = enabled && Boolean(eventId) && !listed && detail.isSuccess && !hosted;
  useEffect(() => {
    if (missing) close();
  }, [missing, close]);

  return { query, events, reminders, viewer, profiles, open, show, close };
}

export type CommunityEventsState = ReturnType<typeof useCommunityEvents>;

/**
 * The community's upcoming events. A plain click on a card opens its detail over this page, as
 * decentraland.org does; the card still links to the event on the events page for a new tab.
 */
export function CommunityEvents({ state }: { state: CommunityEventsState }) {
  const { query, events, reminders, viewer, profiles, show } = state;
  const loadMore = () => void query.fetchNextPage();
  const loader = useLoadMore(
    Boolean(query.hasNextPage) && !query.isFetchNextPageError,
    query.isFetchingNextPage,
    loadMore,
  );
  const { zone, origin, now } = viewer;
  return (
    <CommunityEventsList
      status={listStatus(query)}
      cards={events.map((event) => ({
        ...upcomingCardProps(event, { href: eventHref(event.id), now, zone, origin, pageOrigin: origin, profiles }),
        remind: remindControl(reminders.get(event.id)),
        onOpen: (click) => {
          if (!isPlainClick(click)) return;
          click.preventDefault();
          show(event.id);
        },
      }))}
      notice={reminders.failed ? REMIND_FAILED : undefined}
      onRetry={() => void query.refetch()}
      more={listMore(query, loader, loadMore)}
    />
  );
}

/** The open event's detail, recording its view and Jump In under the events page's story. */
export function CommunityEventDialog({ state, sid }: { state: CommunityEventsState; sid: string }) {
  const { open, viewer, reminders, profiles, close } = state;
  // Opened once hydrated (the viewer's origin is the browser's): Jump In depends on the device.
  if (!open || !viewer.origin) return null;
  return (
    <EventDetail
      sid={sid}
      event={open}
      zone={viewer.zone}
      origin={viewer.origin}
      pageOrigin={viewer.origin}
      jumpIn={(target) => eventJumpIn(target, detectPlatform(navigator.userAgent))}
      reminder={open.live ? undefined : reminders.get(open.id)}
      profiles={profiles}
      onClose={close}
    />
  );
}
