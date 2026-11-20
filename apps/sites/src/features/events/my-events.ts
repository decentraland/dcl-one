import { queryOptions, skipToken } from "@tanstack/react-query";

import type { MyEventCardProps } from "@dcl/one-ui/apps/sites/events/types";
import { eventText, type EventsLocale } from "@dcl/one-ui/apps/sites/events/locale";

import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  editEventHref, eventOccurrence, eventReviewStatus, loadAllOwnedEvents, ownedEventsKeys, type ManagedEvent,
} from "@/data/lib/catalyst/places/event-management";
import type { CreatorProfiles } from "@/data/lib/catalyst/social/creator-profiles";
import type { Zone } from "../components/event-schedule";
import { eventCreator, eventImage, relativeStart } from "./view";

/**
 * The member's own read, every page of it, under `["private", account, "events", "mine"]`: sign-out and
 * an account switch drop it, and `invalidateEventsAfterWrite` refreshes it. Nothing is read without a session.
 */
export function myEventsQuery(identity: AuthIdentity | null) {
  return queryOptions({
    queryKey: ownedEventsKeys.mine(identity?.signer ?? ""),
    queryFn: identity ? ({ signal }) => loadAllOwnedEvents(identity, signal) : skipToken,
  });
}

/**
 * Source's My Events: the member's own events that have not ended, in review or not, soonest first. A
 * recurring event counts by its next occurrence.
 */
export function myEventCards(
  events: readonly ManagedEvent[],
  ctx: { signer: string; now: number; zone: Zone; origin: string; profiles: CreatorProfiles; locale: EventsLocale },
): MyEventCardProps[] {
  const owner = ctx.signer.toLowerCase();
  return events
    .filter((event) => (event.user ?? "").toLowerCase() === owner && !event.deleted_by_user && !event.deleted_by_admin)
    .map((event) => {
      const { start, finish } = eventOccurrence(event);
      return { event, start: start ?? "", finish: finish ?? "" };
    })
    .filter(({ start, finish }) => Number.isFinite(Date.parse(start)) && Date.parse(finish) >= ctx.now)
    .sort((a, b) => Date.parse(a.start) - Date.parse(b.start))
    .map(({ event, start }) => ({
      id: event.id,
      title: event.name || eventText(ctx.locale, "Untitled event"),
      image: eventImage(event.image, ctx.origin),
      href: editEventHref(event.id),
      creator: eventCreator(event, ctx.profiles, ctx.locale),
      time: { label: relativeStart(start, ctx.now, ctx.zone, ctx.locale), dateTime: start },
      status: eventReviewStatus(event),
    }));
}
