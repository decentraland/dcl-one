import type { EventCardCreator, EventCardTime } from "@dcl/one-ui/apps/sites/events/cards/types";

import { eventOccurrence, type Event } from "@/data/lib/catalyst/places/events";
import {
  eventRealm,
  FOUNDATION_CONTACT_NAME,
  googleCalendarUrl,
  isEnsRealm,
  jumpTravelHref,
  parsePosition,
} from "@/data/lib/catalyst/jump/jump";
import type { JumpPlaceRecord } from "@/data/lib/catalyst/jump/jump";
import { DEFAULT_MIN_USERS, type LiveNowData } from "@/data/lib/catalyst/places/events-page";
import { ETH_ADDRESS_RE, shortAddress } from "@/data/lib/catalyst/format/address";
import type { Place } from "@/data/lib/catalyst/schema";
import type { CreatorProfiles } from "@/data/lib/catalyst/social/creator-profiles";

/**
 * The /events page's view model: API records mapped to the card and modal props of
 * `@dcl/one-ui/apps/sites/events`. Times format in a given zone: the server renders UTC and the page
 * switches to the viewer's zone once it hydrates (`timeZone: undefined`).
 */

export const EVENTS_PATH = "/events";
export const EVENT_ID_PARAM = "id";
export const PLACE_POSITION_PARAM = "position";
export const PLACE_WORLD_PARAM = "world";
export const TAB_PARAM = "tab";
export const DAY_PARAM = "day";

const MINUTE = 60_000;
const DAY_MS = 86_400_000;

export type Zone = string | undefined;

/** A travel target: a Genesis City parcel, or a World (with its scene parcel). */
export type Target = { coordinates: [number, number]; realm?: string };

export function eventTarget(event: Event): Target {
  const x = event.x ?? event.position?.[0] ?? 0;
  const y = event.y ?? event.position?.[1] ?? 0;
  return { coordinates: [x, y], realm: eventRealm(event) };
}

export function placeTarget(place: Pick<Place, "base_position" | "world" | "world_name">): Target {
  const worldName = place.world_name?.toLowerCase();
  return {
    coordinates: parsePosition(place.base_position).coordinates,
    realm: place.world && isEnsRealm(worldName) ? worldName : undefined,
  };
}

export function playHref(target: Target): string {
  return jumpTravelHref(target);
}

function profileHref(address: string): string {
  return `/profile/${encodeURIComponent(address.toLowerCase())}`;
}

/** The URL that opens an event's detail on this page, keeping the page's other parameters. */
export function eventHref(id: string, current = ""): string {
  const params = new URLSearchParams(current);
  params.delete(PLACE_POSITION_PARAM);
  params.delete(PLACE_WORLD_PARAM);
  params.set(EVENT_ID_PARAM, id);
  return `${EVENTS_PATH}?${params}`;
}

/** The URL that opens a place's detail: a World by name, else its parcel. */
export function placeHref(target: Target, current = ""): string {
  const params = new URLSearchParams(current);
  params.delete(EVENT_ID_PARAM);
  params.delete(PLACE_POSITION_PARAM);
  params.delete(PLACE_WORLD_PARAM);
  if (target.realm) params.set(PLACE_WORLD_PARAM, target.realm);
  else params.set(PLACE_POSITION_PARAM, target.coordinates.join(","));
  return `${EVENTS_PATH}?${params}`;
}

/** The page URL without any open detail. */
export function closedHref(current: string): string {
  const params = new URLSearchParams(current);
  params.delete(EVENT_ID_PARAM);
  params.delete(PLACE_POSITION_PARAM);
  params.delete(PLACE_WORLD_PARAM);
  const query = params.toString();
  return query ? `${EVENTS_PATH}?${query}` : EVENTS_PATH;
}

/** What links share: a live event opens its jump page, a later one this page's detail, as source. */
export function eventPageShareUrl(origin: string, event: Pick<Event, "id" | "live">): string {
  const url = new URL(event.live ? "/jump/events" : EVENTS_PATH, origin);
  url.searchParams.set(EVENT_ID_PARAM, event.id);
  return url.toString();
}

export function placeShareUrl(origin: string, target: Target): string {
  return new URL(placeHref(target), origin).toString();
}

/** The face a batched profile read found for this wallet, when it found one. */
function faceOf(address: string | undefined, profiles: CreatorProfiles): { avatarUrl?: string } {
  const avatarUrl = address ? profiles[address.toLowerCase()]?.avatarUrl : undefined;
  return avatarUrl ? { avatarUrl } : {};
}

export function eventCreator(
  event: Pick<Event, "user" | "user_name">,
  profiles: CreatorProfiles = {},
): EventCardCreator {
  const address = event.user && ETH_ADDRESS_RE.test(event.user) ? event.user : undefined;
  return {
    name: event.user_name || (address ? shortAddress(address) : "Unknown"),
    href: address ? profileHref(address) : undefined,
    ...faceOf(address, profiles),
  };
}

export function placeCreator(
  place: Pick<JumpPlaceRecord, "owner" | "contact_name">,
  profiles: CreatorProfiles = {},
): EventCardCreator {
  const owner = place.owner && ETH_ADDRESS_RE.test(place.owner) ? place.owner : undefined;
  const name = place.contact_name || (owner ? shortAddress(owner) : place.owner) || "Unknown";
  // The Foundation keeps its logo, as source does for Genesis Plaza.
  if (name === FOUNDATION_CONTACT_NAME) return { name, href: owner ? profileHref(owner) : undefined, foundation: true };
  return { name, href: owner ? profileHref(owner) : undefined, ...faceOf(owner, profiles) };
}

/** The wallets behind a list of events' and places' by-lines, for one batched profile read. */
export function creatorWallets(items: readonly ({ user?: string | null } | { owner: string | null })[]): string[] {
  return items
    .map((item) => ("owner" in item ? item.owner : item.user) ?? "")
    .filter((value) => ETH_ADDRESS_RE.test(value));
}

/** Distinct wallets in the event's scene now; undefined when the record does not say. */
export function eventUsers(event: Pick<Event, "connected_addresses">): number | undefined {
  return event.connected_addresses
    ? new Set(event.connected_addresses.map((address) => address.toLowerCase())).size
    : undefined;
}

export function usersLabel(count: number): string {
  return count === 1 ? "1 online" : `${count} online`;
}

export type LiveNowItem =
  { kind: "event"; event: Event; users: number } | { kind: "place"; place: Place; users: number };

/** The busy place a live event runs in: the World it names, else the place covering its parcel. */
function placeOf(event: Event, places: readonly Place[]): Place | undefined {
  const target = eventTarget(event);
  if (target.realm) return places.find((place) => place.world && place.world_name?.toLowerCase() === target.realm);
  const parcel = target.coordinates.join(",");
  return places.find((place) => !place.world && place.positions.includes(parcel));
}

/**
 * Source's Live Now: places with at least `minUsers` players, each shown as the live event running
 * there when there is one: events first, then the other places, each busiest first. A live event counts its place's players, so an event in an
 * empty scene does not make the rail.
 */
export function liveNowItems(data: LiveNowData, minUsers = DEFAULT_MIN_USERS): LiveNowItem[] {
  const busy = data.places.filter((place) => (place.user_count ?? 0) >= minUsers);
  const used = new Set<string>();
  const events: LiveNowItem[] = [];
  for (const event of data.events) {
    if (!event.live) continue;
    const place = placeOf(event, busy);
    if (!place || used.has(place.id)) continue;
    used.add(place.id);
    events.push({ kind: "event", event, users: place.user_count ?? 0 });
  }
  const places: LiveNowItem[] = busy
    .filter((place) => !used.has(place.id))
    .map((place) => ({ kind: "place" as const, place, users: place.user_count ?? 0 }));
  const busiest = (a: LiveNowItem, b: LiveNowItem) => b.users - a.users;
  return [...events.sort(busiest), ...places.sort(busiest)];
}

/** A calendar day in a zone, as YYYY-MM-DD. */
export function dayKey(instant: Date | number, timeZone: Zone): string {
  return new Intl.DateTimeFormat("en-CA", { timeZone, year: "numeric", month: "2-digit", day: "2-digit" }).format(
    instant,
  );
}

/** The `count` calendar days from `offset` days after today, in the zone. */
export function visibleDays(now: number, timeZone: Zone, offset: number, count: number): { key: string; date: Date }[] {
  const [year, month, day] = dayKey(now, timeZone).split("-").map(Number);
  return Array.from({ length: count }, (_, i) => {
    const date = new Date(Date.UTC(year!, month! - 1, day! + offset + i));
    return { key: date.toISOString().slice(0, 10), date };
  });
}

/** "Today", "Tomorrow", else "Fri, Nov 13", as source's date bar. */
export function dayLabel(date: Date, offset: number): string {
  if (offset === 0) return "Today";
  if (offset === 1) return "Tomorrow";
  return date.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric", timeZone: "UTC" });
}

export function dayFullLabel(date: Date): string {
  return date.toLocaleDateString("en-US", {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });
}

export function formatTime(iso: string, timeZone: Zone): string {
  return new Intl.DateTimeFormat("en-US", { hour: "numeric", minute: "2-digit", hour12: true, timeZone }).format(
    new Date(iso),
  );
}

function formatDate(iso: string, timeZone: Zone): string {
  return new Intl.DateTimeFormat("en-US", { weekday: "short", month: "short", day: "numeric", timeZone }).format(
    new Date(iso),
  );
}

/** Source's Upcoming label: "Starts in 8 mins", "Today 5:00 PM", "Tomorrow 1:00 AM", else the time. */
export function relativeStart(iso: string, now: number, timeZone: Zone): string {
  const start = Date.parse(iso);
  const diff = start - now;
  if (diff <= 0) return formatTime(iso, timeZone);
  const minutes = Math.round(diff / MINUTE);
  if (minutes < 60) return `Starts in ${minutes} ${minutes === 1 ? "min" : "mins"}`;
  const day = dayKey(start, timeZone);
  if (day === dayKey(now, timeZone)) return `Today ${formatTime(iso, timeZone)}`;
  if (day === dayKey(now + DAY_MS, timeZone)) return `Tomorrow ${formatTime(iso, timeZone)}`;
  return `${formatDate(iso, timeZone)} ${formatTime(iso, timeZone)}`;
}

export type Occurrence = { event: Event; start: string; finish: string | undefined; live: boolean };

/**
 * The event's occurrences inside the visible days: every listed date of a recurring event, else its
 * next start. Occurrences that already ended are left out, as source drops them from Today.
 */
export function occurrences(event: Event, now: number): Occurrence[] {
  const { start, finish } = eventOccurrence(event);
  const duration = start && finish ? Date.parse(finish) - Date.parse(start) : undefined;
  const dates = event.recurrent && event.recurrent_dates?.length ? event.recurrent_dates : start ? [start] : [];
  const out: Occurrence[] = [];
  for (const date of dates) {
    const at = Date.parse(date);
    if (Number.isNaN(at)) continue;
    const end = duration !== undefined && duration > 0 ? at + duration : undefined;
    if (end !== undefined && end < now) continue;
    const live = date === start ? event.live : end !== undefined && at <= now && now <= end;
    out.push({
      event,
      start: new Date(at).toISOString(),
      finish: end === undefined ? undefined : new Date(end).toISOString(),
      live,
    });
  }
  return out;
}

/** Occurrences grouped by the given day keys, each day in start order. */
export function bucketByDay(
  events: readonly Event[],
  days: readonly string[],
  now: number,
  timeZone: Zone,
): Occurrence[][] {
  const index = new Map(days.map((key, i) => [key, i]));
  const buckets: Occurrence[][] = days.map(() => []);
  for (const event of events) {
    for (const occurrence of occurrences(event, now)) {
      const i = index.get(dayKey(Date.parse(occurrence.start), timeZone));
      if (i !== undefined) buckets[i]!.push(occurrence);
    }
  }
  for (const bucket of buckets) bucket.sort((a, b) => Date.parse(a.start) - Date.parse(b.start));
  return buckets;
}

const FREQUENCY: Record<string, [one: string, many: (n: number) => string]> = {
  DAILY: ["Repeats daily", (n) => `Every ${n} days`],
  WEEKLY: ["Repeats weekly", (n) => `Every ${n} weeks`],
  MONTHLY: ["Repeats monthly", (n) => `Every ${n} months`],
  YEARLY: ["Repeats yearly", (n) => `Every ${n} years`],
};

export function recurrenceLabel(
  event: Pick<Event, "recurrent" | "recurrent_frequency" | "recurrent_interval">,
): string | undefined {
  if (!event.recurrent || !event.recurrent_frequency) return undefined;
  const labels = FREQUENCY[event.recurrent_frequency];
  if (!labels) return undefined;
  const interval = event.recurrent_interval ?? 1;
  return interval > 1 ? labels[1](interval) : labels[0];
}

/** The hero line: "Wed, Nov 11 - 5:00 PM", or "Every Wednesday - 5:00 PM" for a weekly event. */
export function heroWhen(event: Event, timeZone: Zone): EventCardTime | undefined {
  const { start } = eventOccurrence(event);
  if (!start || Number.isNaN(Date.parse(start))) return undefined;
  const time = formatTime(start, timeZone);
  if (event.recurrent && event.recurrent_frequency === "WEEKLY" && (event.recurrent_interval ?? 1) === 1) {
    const weekday = new Intl.DateTimeFormat("en-US", { weekday: "long", timeZone }).format(new Date(start));
    return { label: `Every ${weekday} - ${time}`, dateTime: start };
  }
  if (event.recurrent && event.recurrent_frequency === "DAILY" && (event.recurrent_interval ?? 1) === 1) {
    return { label: `Every day - ${time}`, dateTime: start };
  }
  return { label: `${formatDate(start, timeZone)} - ${time}`, dateTime: start };
}

/** The schedule line: "Wed, Nov 11 · 5:00 PM – 7:00 PM", prefixed "Starting" for a recurring event. */
export function scheduleLine(event: Event, timeZone: Zone): (EventCardTime & { recurrence?: string }) | undefined {
  const { start, finish } = eventOccurrence(event);
  if (!start || Number.isNaN(Date.parse(start))) return undefined;
  const range = `${formatDate(start, timeZone)} · ${formatTime(start, timeZone)}${finish ? ` – ${formatTime(finish, timeZone)}` : ""}`;
  return {
    label: event.recurrent ? `Starting ${range}` : range,
    dateTime: start,
    recurrence: recurrenceLabel(event),
  };
}

export function calendarHref(
  event: Event,
  shareUrl: string,
  occurrence?: { start: string; finish?: string },
): string | undefined {
  const { start, finish } = occurrence ?? eventOccurrence(event);
  return (
    googleCalendarUrl(
      {
        title: event.name ?? "Untitled event",
        description: event.description ?? undefined,
        startAt: start ?? null,
        finishAt: finish ?? null,
      },
      shareUrl,
    ) ?? undefined
  );
}

export function locationLabel(target: Target, sceneName?: string | null): string {
  if (target.realm) return target.realm;
  const coords = `(${target.coordinates.join(",")})`;
  return sceneName ? `${sceneName} ${coords}` : coords;
}

/**
 * Where an old What's On or events-site link lands: this page with the link's query, and a
 * `/whats-on/<id>` detail path as the event the page opens (`?id=`).
 */
export function legacyEventsHref(url: URL, eventId?: string): string {
  const params = new URLSearchParams(url.search);
  if (eventId) params.set(EVENT_ID_PARAM, eventId);
  const query = params.toString();
  return query ? `${EVENTS_PATH}?${query}` : EVENTS_PATH;
}

/**
 * A deeper What's On path. The moderation and edit links the events service still sends (Slack
 * notices) keep their target: `/whats-on/admin/*` is `/events/admin/*` and `/whats-on/edit-hangout/:id`
 * is `/events/edit-event/:id`; anything else lands on this page. The query is kept either way.
 */
export function legacyWhatsOnHref(url: URL, splat: string): string {
  const segments = splat.split("/").filter(Boolean);
  if (segments[0] === "admin" && segments.length > 1) {
    return `${EVENTS_PATH}/admin/${segments.slice(1).map(encodeURIComponent).join("/")}${url.search}`;
  }
  if (segments[0] === "edit-hangout" && segments.length === 2) {
    return `${EVENTS_PATH}/edit-event/${encodeURIComponent(segments[1]!)}${url.search}`;
  }
  return legacyEventsHref(url);
}

/** All Events pages up to a year ahead. */
export const MAX_DAY_OFFSET = 365;

/** The All Events day offset in the URL: whole days from today, never negative, at most a year. */
export function readDayOffset(params: URLSearchParams): number {
  const value = Number(params.get(DAY_PARAM) ?? 0);
  return Number.isSafeInteger(value) && value > 0 ? Math.min(value, MAX_DAY_OFFSET) : 0;
}
