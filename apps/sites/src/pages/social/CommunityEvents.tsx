import { useInfiniteQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";

import { CommunityEventsList } from "@dcl/one-ui/apps/sites/social/CommunityLists";

import { useAuth } from "@/data/lib/auth/context";
import type { Event } from "@/data/lib/catalyst/places/events";
import { eventsQuery } from "@/data/lib/community/queries";
import { openSignIn } from "@/features/components/auth/signin-store";
import type { Zone } from "@/features/components/event-schedule";
import { detectPlatform } from "@/features/download/options";
import { EventDetail, eventJumpIn, isPlainClick, remindControl } from "@/features/events/EventDetail";
import { useCreatorProfiles } from "@/features/events/useCreatorProfiles";
import { REMIND_FAILED, useEventReminders } from "@/features/events/useEventReminders";
import { creatorWallets, eventHref, upcomingCardProps } from "@/features/events/view";
import { useLoadMore } from "@/features/social/community/useLoadMore";

import { listMore, listStatus } from "./list-state";

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
 * The community's upcoming events. A plain click on a card opens its detail over this page, as
 * decentraland.org does; the card still links to the event on the events page for a new tab.
 */
export function CommunityEvents({ communityId, now: initialNow }: { communityId: string; now: number }) {
  const { identity } = useAuth();
  const query = useInfiniteQuery(eventsQuery(communityId));
  const events = useMemo(() => query.data?.pages.flatMap((page) => page.events) ?? [], [query.data]);
  const groups = useMemo(() => [creatorWallets(events)], [events]);
  const profiles = useCreatorProfiles(groups);
  const reminders = useEventReminders(identity, () => openSignIn());
  const loadMore = () => void query.fetchNextPage();
  const loader = useLoadMore(
    Boolean(query.hasNextPage) && !query.isFetchNextPageError,
    query.isFetchingNextPage,
    loadMore,
  );
  const { zone, origin, now } = useViewer(initialNow);
  const [open, setOpen] = useState<Event | null>(null);
  return (
    <>
      <CommunityEventsList
        status={listStatus(query)}
        cards={events.map((event) => ({
          ...upcomingCardProps(event, { href: eventHref(event.id), now, zone, origin, pageOrigin: origin, profiles }),
          remind: remindControl(reminders.get(event.id)),
          onOpen: (click) => {
            if (!isPlainClick(click)) return;
            click.preventDefault();
            setOpen(event);
          },
        }))}
        notice={reminders.failed ? REMIND_FAILED : undefined}
        onRetry={() => void query.refetch()}
        more={listMore(query, loader, loadMore)}
      />
      {open ? (
        <EventDetail
          // The events page's detail telemetry names its own story, so this page records none.
          sid=""
          event={open}
          zone={zone}
          origin={origin}
          pageOrigin={origin}
          // The dialog opens only after a click, so the browser's own user agent is known.
          jumpIn={(target) => eventJumpIn(target, detectPlatform(navigator.userAgent))}
          reminder={open.live ? undefined : reminders.get(open.id)}
          profiles={profiles}
          onClose={() => setOpen(null)}
        />
      ) : null}
    </>
  );
}
