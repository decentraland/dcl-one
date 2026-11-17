import { useInfiniteQuery } from "@tanstack/react-query";
import { useEffect, useId, useMemo, useState } from "react";

import { Spinner } from "@dcl/one-ui";
import { UpcomingCard } from "@dcl/one-ui/apps/sites/events/cards";
import { GeneralCalendar } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/context";
import { eventsQuery } from "@/data/lib/community/queries";

import { openSignIn } from "../../components/auth/signin-store";
import { creatorWallets, type Zone } from "../../events/view";
import { useCreatorProfiles } from "../../events/useCreatorProfiles";
import { useEventReminders } from "../../events/useEventReminders";
import styles from "./CommunityLists.module.css";
import { useLoadMore } from "./useLoadMore";
import { eventCard } from "./view";

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
  useEffect(() => setViewer({ zone: undefined, origin: window.location.origin, now: Date.now() }), []);
  return viewer;
}

export function CommunityEvents({ communityId, now: initialNow }: { communityId: string; now: number }) {
  const titleId = useId();
  const { identity } = useAuth();
  const query = useInfiniteQuery(eventsQuery(communityId));
  const events = useMemo(() => query.data?.pages.flatMap((page) => page.events) ?? [], [query.data]);
  const groups = useMemo(() => [creatorWallets(events)], [events]);
  const profiles = useCreatorProfiles(groups);
  const reminders = useEventReminders(identity, () => openSignIn({ inPlace: true }));
  const sentinel = useLoadMore(Boolean(query.hasNextPage), query.isFetchingNextPage, () => void query.fetchNextPage());
  const { zone, origin, now } = useViewer(initialNow);
  return (
    <section className={styles.section} aria-labelledby={titleId} aria-busy={query.isPending || undefined}>
      <h2 id={titleId} className={styles.title}>
        Upcoming Events
      </h2>
      {query.isPending ? (
        <div className={styles.loader}>
          <Spinner variant="light" label="Loading events" />
        </div>
      ) : query.isError && events.length === 0 ? (
        <p className={styles.empty} role="alert">
          We couldn't load the events right now.
        </p>
      ) : events.length === 0 ? (
        <div className={styles.emptyEvents}>
          <GeneralCalendar size={48} aria-hidden="true" />
          <p>No Upcoming Events</p>
        </div>
      ) : (
        <ul className={styles.events}>
          {events.map((event) => {
            const reminder = reminders.get(event.id);
            return (
              <li key={event.id}>
                <UpcomingCard
                  {...eventCard(event, { now, zone, origin, profiles })}
                  remind={{ active: reminder.active, pending: reminder.pending, onToggle: reminder.toggle }}
                />
              </li>
            );
          })}
        </ul>
      )}
      {query.hasNextPage ? (
        <div ref={sentinel} className={styles.sentinel}>
          {query.isFetchingNextPage ? <Spinner variant="light" label="Loading more events" /> : null}
        </div>
      ) : null}
    </section>
  );
}
