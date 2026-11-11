import type { JumpCardProps } from "@dcl/one-ui/apps/sites/jump/JumpCard";

import {
  eventHasEnded,
  formatEventStart,
  formatLocation,
  FOUNDATION_CONTACT_NAME,
  jumpTravelHref,
  type JumpCard,
} from "@/data/lib/catalyst/jump/jump";
import type { JumpCreator } from "@/data/lib/catalyst/jump/jump.server";

/** The serializable part of the card; event actions are added by the page. */
export type JumpCardView = Omit<JumpCardProps, "actions">;

export function profileHref(address: string): string {
  return `/profile/${encodeURIComponent(address.toLowerCase())}`;
}

/**
 * The card as the page shows it. A creator resolved from the scene's deployer replaces the catalogue's
 * contact name; an event card shows Jump In only while it is live.
 */
export function cardProps(card: JumpCard, creator: JumpCreator | null, now = Date.now()): JumpCardView {
  const address = creator?.address ?? card.creatorAddress;
  const foundation = card.kind === "place" && card.creatorName === FOUNDATION_CONTACT_NAME;
  const props: JumpCardView = {
    kind: card.kind,
    title: card.title,
    creator: {
      name: creator?.name || card.creatorName,
      avatar: creator?.avatar,
      foundation,
      href: address ? profileHref(address) : undefined,
    },
    image: card.image,
    description: card.description,
    location: { world: card.realm, parcel: formatLocation(card.coordinates) },
  };
  if (!card.event) {
    if (card.userCount > 0) props.badge = { kind: "players", count: card.userCount };
    props.jumpInHref = jumpTravelHref(card);
    return props;
  }
  const { live, startAt, finishAt, totalAttendees } = card.event;
  const ended = eventHasEnded(finishAt, now);
  // The server renders UTC; the page switches to the viewer's own zone once it hydrates.
  const start = formatEventStart(startAt, "UTC");
  if (start && startAt) {
    props.when = ended ? { label: "Event has ended", ended: true } : { label: start, ended: false, dateTime: startAt };
  }
  props.badge = live ? { kind: "live", count: card.userCount } : { kind: "attendees", count: totalAttendees };
  if (live) props.jumpInHref = jumpTravelHref(card);
  return props;
}

export type EventActionState = "none" | "deleted" | "ended" | "live" | "upcoming";

/** An event without a start offers nothing to plan for: only Jump In while it is live. */
export function eventActionState(card: JumpCard, now = Date.now()): EventActionState {
  if (!card.event) return "none";
  if (!card.event.startAt) return card.event.live ? "live" : "none";
  if (card.event.deleted) return "deleted";
  if (card.event.live) return "live";
  return eventHasEnded(card.event.finishAt, now) ? "ended" : "upcoming";
}
