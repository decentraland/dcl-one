import type { MetaDescriptor } from "react-router";

import { readJumpTarget } from "@/data/lib/catalyst/jump/jump";
import type { Event } from "@/data/lib/catalyst/places/events";
import type { Place } from "@/data/lib/catalyst/schema";

import { SITE_NAME, absoluteUrl, notFoundMeta, oneLine, socialMeta, SCHEMA_ORG, absoluteImage } from "./social-meta";

/**
 * Link-preview cards for events, jump and places pages, as decentraland.org's destinations handler
 * wrote them (sites-deployer `OpenGraphDestinationsRoute`, `destinationDocument`, `eventsListing`).
 */

type Section = { title: string; description: string };

/** Every events, What's On and jump URL without a target of its own. */
export const EVENTS_SECTION: Section = {
  title: "Events in Decentraland",
  description: "Live events and places happening right now in Decentraland.",
};

export const PLACES_SECTION: Section = {
  title: "Places in Decentraland",
  description: "Explore the worlds, games and scenes people are in right now in Decentraland.",
};

export type DestinationCard = { title: string; description: string; image?: string };

/** The fields a card reads from a place or World catalogue row. */
type CatalogueRow = { title: string | null; description: string | null; image: string | null };


export function sectionCard(section: Section): DestinationCard {
  return { title: section.title, description: section.description };
}

/** `{name} at {scene}`; the events section's description and image when the event has none. */
export function eventCard(event: Pick<Event, "name" | "description" | "image"> & { scene_name?: string | null }): DestinationCard {
  const scene = oneLine(event.scene_name);
  return {
    title: `${oneLine(event.name) ?? EVENTS_SECTION.title}${scene ? ` at ${scene}` : ""}`,
    description: oneLine(event.description) ?? EVENTS_SECTION.description,
    image: absoluteImage(undefined, event.image),
  };
}

/** A World by its name, with or without a catalogue row. */
export function worldCard(name: string, row?: CatalogueRow | null): DestinationCard {
  return {
    title: oneLine(row?.title) ?? `Visit ${name} in Decentraland`,
    description: oneLine(row?.description) ?? `Discover ${name}, a Decentraland world.`,
    image: absoluteImage(undefined, row?.image),
  };
}

/** A Genesis City parcel (`x,y`), with or without the place that serves it. */
export function parcelCard(position: string, row?: CatalogueRow | null): DestinationCard {
  return {
    title: oneLine(row?.title) ?? `Explore (${position}) in Decentraland`,
    description: oneLine(row?.description) ?? `Discover what's happening at coordinates ${position} in Decentraland.`,
    image: absoluteImage(undefined, row?.image),
  };
}

/** The page head for a card: every title reads `{title} | Decentraland`. */
export function destinationMeta(card: DestinationCard, options: { url?: string; jsonLd?: readonly object[] } = {}): MetaDescriptor[] {
  return socialMeta({ title: `${card.title} | ${SITE_NAME}`, description: card.description, image: card.image, ...options });
}


const isoDate = (value: string | null | undefined) => {
  if (!value) return undefined;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
};

/** An occurrence: the next one of a recurrent event, else the event's own dates. */
function occurrence(event: Event): { start?: string; end?: string } {
  const next = isoDate(event.next_start_at);
  return next ? { start: next, end: isoDate(event.next_finish_at) } : { start: isoDate(event.start_at), end: isoDate(event.finish_at) };
}

/** schema.org Event for a published (approved, not rejected) event; null otherwise. */
export function eventJsonLd(event: Event, url: string): object | null {
  const name = oneLine(event.name);
  if (!name || event.approved !== true || event.rejected === true) return null;
  const scene = oneLine(event.scene_name);
  // Only a Genesis City event has a parcel; inside a World the coordinates are the World's own.
  const parcel = event.world !== true && event.x != null && event.y != null ? `${event.x},${event.y}` : null;
  const where = [scene, parcel ? `Genesis City parcel ${parcel}` : null].filter(Boolean).join(", ");
  const { start, end } = occurrence(event);
  const description = oneLine(event.description);
  const image = absoluteImage(undefined, event.image);
  const organizer = oneLine(event.user_name);
  return {
    "@context": SCHEMA_ORG,
    "@type": "Event",
    name,
    url,
    eventAttendanceMode: `${SCHEMA_ORG}/OnlineEventAttendanceMode`,
    eventStatus: `${SCHEMA_ORG}/EventScheduled`,
    location: { "@type": "VirtualLocation", url, ...(where ? { name: where } : {}) },
    ...(description ? { description } : {}),
    ...(start ? { startDate: start } : {}),
    ...(start && end ? { endDate: end } : {}),
    ...(image ? { image } : {}),
    ...(organizer ? { organizer: { "@type": "Person", name: organizer } } : {}),
  };
}

/** schema.org Place for a place or World page; null for a row without a title. */
export function placeJsonLd(place: Place, url: string): object | null {
  const name = oneLine(place.title);
  if (!name) return null;
  const description = oneLine(place.description);
  const image = absoluteImage(undefined, place.image);
  const keywords = place.categories.filter((category) => category.trim()).join(", ");
  const worldName = place.world ? oneLine(place.world_name)?.toLowerCase() : undefined;
  const parcel = place.world ? undefined : oneLine(place.base_position) ?? oneLine(place.positions[0]);
  return {
    "@context": SCHEMA_ORG,
    "@type": "Place",
    name,
    url,
    ...(description ? { description } : {}),
    ...(image ? { image } : {}),
    ...(keywords ? { keywords } : {}),
    ...(worldName ? { additionalProperty: [{ "@type": "PropertyValue", name: "world", value: worldName }] } : {}),
    ...(parcel
      ? {
          containedInPlace: { "@type": "Place", name: "Genesis City" },
          additionalProperty: [{ "@type": "PropertyValue", name: "parcel", value: parcel }],
        }
      : {}),
  };
}

/** A place page's canonical path, from the row: a World by its name, a parcel place by its id. */
export function placeCanonicalPath(place: Place): string {
  const world = place.world ? place.world_name?.toLowerCase() : undefined;
  return world ? `/places/world/${encodeURIComponent(world)}` : `/places/${encodeURIComponent(place.id)}`;
}

/**
 * A jump page's canonical: its path with the one target the card describes, an event id (event jump
 * only), else the World, else a valid parcel; tracking and other parameters are dropped.
 */
export function jumpCanonicalPath(pathname: string, params: URLSearchParams, kind: "event" | "place"): string {
  const path = pathname.replace(/\/+$/, "") || "/";
  const target = readJumpTarget(params);
  if (kind === "event" && target.id && target.validId) return `${path}?id=${encodeURIComponent(target.id)}`;
  if (target.realm) return `${path}?world=${encodeURIComponent(target.realm)}`;
  const position = jumpPosition(params);
  return position ? `${path}?position=${position}` : path;
}

/** The parcel a jump link names, normalized to `x,y`; undefined when it names none or a malformed one. */
export function jumpPosition(params: URLSearchParams): string | undefined {
  const target = readJumpTarget(params);
  return params.get("position")?.trim() && target.validPosition ? target.coordinates.join(",") : undefined;
}

/** An event page's canonical: the events page opened on it. */
export const eventCanonicalPath = (id: string) => `/events?id=${encodeURIComponent(id)}`;

/**
 * The /events listing's ItemLists, from the live and upcoming events its first document already holds
 * (the page's reads keep only published events).
 */
export function eventsListingJsonLd(base: string, lists: { name: string; events: readonly Event[] }[]): object {
  const graph = lists.map(({ name, events }) => {
    const items = events.flatMap((event) => {
      const title = oneLine(event.name);
      const url = absoluteUrl(base, eventCanonicalPath(event.id));
      return title && url ? [{ url, name: title }] : [];
    });
    return {
      "@type": "ItemList",
      name,
      numberOfItems: items.length,
      itemListElement: items.map((item, index) => ({ "@type": "ListItem", position: index + 1, ...item })),
    };
  });
  return { "@context": SCHEMA_ORG, "@graph": graph };
}

/** The head of a place page, shared with the World route; `unavailable` is the card while the catalogue is down. */
export function placeDetailMeta(
  data: { place: Place | null; unavailable: boolean } | undefined,
  base: string | undefined,
  unavailable: DestinationCard,
  unavailablePath: string,
) {
  const place = data?.place;
  if (place) {
    const url = absoluteUrl(base, placeCanonicalPath(place));
    const card = place.world && place.world_name ? worldCard(place.world_name.toLowerCase(), place) : parcelCard(place.base_position, place);
    const jsonLd = url ? placeJsonLd(place, url) : null;
    return destinationMeta(card, { url, jsonLd: jsonLd ? [jsonLd] : undefined });
  }
  if (data && !data.unavailable) return notFoundMeta();
  return destinationMeta(unavailable, { url: absoluteUrl(base, unavailablePath) });
}
