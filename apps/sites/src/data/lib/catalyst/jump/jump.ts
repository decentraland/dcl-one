import { sceneTravelHref } from "../places/world-scenes";

export const DEFAULT_POSITION = "0,0";
export const DEFAULT_REALM = "main";

export type ParsedPosition = { coordinates: [number, number]; isValid: boolean };

/** "x,y" and "x.y" are the same pair: the dot is a separator, not a decimal. */
export function parsePosition(value: string): ParsedPosition {
  const tokens = value.split(/[,.]/);
  if (tokens.length !== 2) return { coordinates: [0, 0], isValid: false };
  const x = Number.parseInt(tokens[0], 10);
  const y = Number.parseInt(tokens[1], 10);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return { coordinates: [0, 0], isValid: false };
  return { coordinates: [x, y], isValid: true };
}

const ENS = /^[a-zA-Z0-9._-]+\.eth$/;

export function isEnsRealm(value: string | undefined): value is `${string}.eth` {
  return value !== undefined && ENS.test(value);
}

export type JumpTarget = {
  /** The pair to show and travel to; `0,0` when the link carries none. */
  coordinates: [number, number];
  /** False when the link carries a position that is not a coordinate pair. */
  validPosition: boolean;
  /** The pair to look up, or undefined for a World link without one (the World-level record). */
  lookup: [number, number] | undefined;
  /** Undefined for Genesis City (`main` or no realm). */
  realm: string | undefined;
  id: string | null;
};

/** `?world=` is the alias older clients emit for `?realm=`. */
export function readJumpTarget(params: URLSearchParams): JumpTarget {
  const rawPosition = params.get("position");
  const parsed = parsePosition(rawPosition ?? DEFAULT_POSITION);
  const realmParam = params.get("realm") ?? params.get("world") ?? DEFAULT_REALM;
  const realm = realmParam === DEFAULT_REALM ? undefined : realmParam;
  return {
    coordinates: parsed.coordinates,
    validPosition: parsed.isValid,
    lookup: rawPosition !== null || !realm ? parsed.coordinates : undefined,
    realm,
    id: params.get("id")?.trim() || null,
  };
}

export type JumpCard = {
  kind: "place" | "event";
  title: string;
  /** The wallet behind the creator name, when there is one. */
  creatorAddress?: string;
  creatorName: string;
  image?: string;
  description?: string;
  coordinates: [number, number];
  realm?: string;
  userCount: number;
  /** Event only. */
  event?: {
    id: string;
    startAt: string | null;
    finishAt: string | null;
    live: boolean;
    totalAttendees: number;
    deleted: boolean;
  };
};

export type JumpPlaceRecord = {
  title: string | null;
  description: string | null;
  image: string | null;
  base_position: string;
  owner: string | null;
  contact_name: string | null;
  user_count: number | null;
  world: boolean;
  world_name: string | null;
};

export const FOUNDATION_CONTACT_NAME = "Decentraland Foundation";
const ETH_ADDRESS = /^0x[0-9a-fA-F]{40}$/;

/**
 * The scene's own contact wins over `owner`: the owner is whoever holds the land or name.
 * A wallet `owner` never stands in as the name; a non-wallet `owner` is a display string and can.
 */
export function toPlaceCard(place: JumpPlaceRecord): JumpCard {
  const coordinates = parsePosition(place.base_position).coordinates;
  const ownerAddress = place.owner && ETH_ADDRESS.test(place.owner) ? place.owner : undefined;
  return {
    kind: "place",
    title: place.title ?? "",
    creatorAddress: ownerAddress,
    creatorName: place.contact_name || (ownerAddress ? "" : place.owner) || "Unknown",
    image: place.image || undefined,
    description: place.description || undefined,
    coordinates,
    realm: place.world ? (place.world_name ?? undefined) : undefined,
    userCount: place.user_count ?? 0,
  };
}

/** A parcel or World with no catalogue entry still gets a card to jump from. */
export function genericPlaceCard(coordinates: [number, number], realm: string | undefined): JumpCard {
  return { kind: "place", title: realm ?? "", creatorName: "Unknown", coordinates, realm, userCount: 0 };
}

export type JumpEventRecord = {
  id: string;
  name: string;
  image: string | null;
  description: string | null;
  next_start_at: string | null;
  next_finish_at: string | null;
  coordinates: [number, number];
  user: string | null;
  user_name: string | null;
  total_attendees: number;
  live: boolean;
  world: boolean;
  server: string | null;
  deleted_by_user: boolean;
  deleted_by_admin: boolean;
};

/** A World event travels to its server; a Genesis City one to its parcel. */
export function eventRealm(event: Pick<JumpEventRecord, "world" | "server">): string | undefined {
  return event.world && event.server ? event.server : undefined;
}

/** A deleted event never advertises itself as live or ended. */
export function toEventCard(event: JumpEventRecord): JumpCard {
  const deleted = event.deleted_by_user || event.deleted_by_admin;
  return {
    kind: "event",
    title: event.name,
    creatorAddress: event.user ?? undefined,
    creatorName: event.user_name || "Unknown",
    image: event.image || undefined,
    description: event.description || undefined,
    coordinates: event.coordinates,
    realm: eventRealm(event),
    userCount: 0,
    event: {
      id: event.id,
      startAt: event.next_start_at,
      finishAt: deleted ? null : event.next_finish_at,
      live: deleted ? false : event.live,
      totalAttendees: event.total_attendees,
      deleted,
    },
  };
}

export function eventHasEnded(finishAt: string | null | undefined, now = Date.now()): boolean {
  if (!finishAt) return false;
  const at = new Date(finishAt).getTime();
  return !Number.isNaN(at) && now > at;
}

/** A start time in the given zone, or the runtime's own when none is given. Null for a missing or bad date. */
export function formatEventStart(iso: string | null, timeZone?: string): string | null {
  if (!iso) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short",
    timeZone,
  });
}

export function formatLocation([x, y]: [number, number]): string {
  return `${x}, ${y}`;
}

/** Where Jump In goes: the explorer entry with the target's realm and parcel. */
export function jumpTravelHref(card: Pick<JumpCard, "coordinates" | "realm">): string {
  return sceneTravelHref(card.realm ?? null, card.coordinates.join(","));
}

/** The link an event is shared and added to calendars with. */
export function eventShareUrl(origin: string, card: Pick<JumpCard, "coordinates" | "realm">): string {
  const url = new URL("/jump/events", origin);
  url.searchParams.set("position", card.coordinates.join(","));
  if (card.realm) url.searchParams.set("realm", card.realm);
  return url.toString();
}

function calendarStamp(date: Date): string {
  return date.toISOString().replace(/[-:]/g, "").replace(/\.\d{3}/, "");
}

/** Google Calendar's template link; null when the event has no valid start and finish. */
export function googleCalendarUrl(
  event: { title: string; description?: string; startAt: string | null; finishAt: string | null },
  shareUrl: string,
): string | null {
  if (!event.startAt || !event.finishAt) return null;
  const start = new Date(event.startAt);
  const finish = new Date(event.finishAt);
  if (Number.isNaN(start.getTime()) || Number.isNaN(finish.getTime())) return null;
  const details = event.description ? `${event.description}\n\nJump In: ${shareUrl}` : `Jump In: ${shareUrl}`;
  const params = new URLSearchParams({
    text: event.title,
    details,
    dates: `${calendarStamp(start)}/${calendarStamp(finish)}`,
  });
  return `https://calendar.google.com/calendar/r/eventedit?${params}`;
}
