import { useInfiniteQuery } from "@tanstack/react-query";
import { useEffect, useId, useMemo, useState } from "react";

import { Button, Spinner } from "@dcl/one-ui";
import { UpcomingCard } from "@dcl/one-ui/apps/sites/events/cards";
import { GeneralCalendar } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/context";
import { eventsQuery } from "@/data/lib/community/queries";

import { openSignIn } from "../../components/auth/signin-store";
import type { Zone } from "../../components/event-schedule";
import { creatorWallets, eventHref, upcomingCardProps } from "../../events/view";
import { useCreatorProfiles } from "../../events/useCreatorProfiles";
import { REMIND_FAILED, useEventReminders } from "../../events/useEventReminders";
import styles from "./CommunityLists.module.css";
import { useLoadMore } from "./useLoadMore";

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

export function CommunityEvents({ communityId, now: initialNow }: { communityId: string; now: number }) {
  const titleId = useId();
  const { identity } = useAuth();
  const query = useInfiniteQuery(eventsQuery(communityId));
  const events = useMemo(() => query.data?.pages.flatMap((page) => page.events) ?? [], [query.data]);
  const groups = useMemo(() => [creatorWallets(events)], [events]);
  const profiles = useCreatorProfiles(groups);
  const reminders = useEventReminders(identity, () => openSignIn());
  const loader = useLoadMore(
    Boolean(query.hasNextPage) && !query.isFetchNextPageError,
    query.isFetchingNextPage, () => void query.fetchNextPage());
  const { zone, origin, now } = useViewer(initialNow);
  return (
    <section className={styles.section} aria-labelledby={titleId} aria-busy={query.isPending || undefined}>
      <h2 id={titleId} className={styles.title}>
        Upcoming Events
      </h2>
      {reminders.failed ? (
        <p className={styles.notice} role="alert">
          {REMIND_FAILED}
        </p>
      ) : null}
      {query.isPending ? (
        <div className={styles.loader}>
          <Spinner variant="light" label="Loading events" />
        </div>
      ) : query.isError && events.length === 0 ? (
        <div className={styles.retry} role="alert">
          <p className={styles.empty}>We couldn't load the events right now.</p>
          <Button variant="secondary" size="sm" onClick={() => void query.refetch()}>
            Try again
          </Button>
        </div>
      ) : events.length === 0 ? (
        <div className={styles.emptyEvents}>
          <GeneralCalendar size={64} aria-hidden="true" />
          <p>No Upcoming Events</p>
        </div>
      ) : (
        <ul className={styles.events}>
          {events.map((event) => {
            const reminder = reminders.get(event.id);
            return (
              <li key={event.id}>
                <UpcomingCard
                  {...upcomingCardProps(event, { href: eventHref(event.id), now, zone, origin, profiles })}
                  remind={{ active: reminder.active, pending: reminder.pending, onToggle: reminder.toggle }}
                />
              </li>
            );
          })}
        </ul>
      )}
      {query.isFetchNextPageError ? (
        <div className={styles.retry} role="alert">
          <p className={styles.empty}>We couldn't load more events.</p>
          <Button variant="secondary" size="sm" onClick={() => void query.fetchNextPage()}>
            Try again
          </Button>
        </div>
      ) : query.hasNextPage ? (
        <div ref={loader.ref} className={styles.sentinel}>
          {query.isFetchingNextPage ? (
            <Spinner variant="light" label="Loading more events" />
          ) : loader.manual ? (
            <Button variant="secondary" size="sm" onClick={() => void query.fetchNextPage()}>
              Load more
            </Button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
