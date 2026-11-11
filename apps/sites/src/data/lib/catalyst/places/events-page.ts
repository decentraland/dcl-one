import { queryOptions, type DehydratedState } from "@tanstack/react-query";

import { CatalystError, type GetOptions } from "../client";
import { isEnsRealm, parsePosition, type JumpPlaceRecord } from "../jump/jump";
import { loadJumpPlace } from "../jump/place-lookup";
import type { Place } from "../schema";
import { fetchEvent, fetchEvents, type Event } from "./events";
import { fetchMostActivePlaces } from "./index";

/** The /events page's public reads: one key and options factory shared by its loader and its components. */

export const EVENTS_PAGE_STALE_MS = 30_000;
/** Live Now refreshes while the page is open, as the source polls it every minute. */
export const LIVE_NOW_REFRESH_MS = 60_000;

const LIVE_EVENTS_LIMIT = 20;
const HOT_PLACES_LIMIT = 20;
const UPCOMING_LIMIT = 10;
const UPCOMING_WINDOW_MS = 24 * 60 * 60_000;
/** The source's All Events window: every active event, bucketed by day on the client. */
export const CALENDAR_LIMIT = 200;
/** A place joins Live Now from this many players, as the source's hot-scene threshold. */
export const DEFAULT_MIN_USERS = 5;

/** A place link names a parcel (`position`) or a World (`world`). */
export type PlaceTarget = { position: [number, number] } | { world: string };

export const eventsPageKeys = {
  all: ["events"] as const,
  live: () => ["events", "live"] as const,
  upcoming: () => ["events", "upcoming"] as const,
  calendar: () => ["events", "calendar"] as const,
  detail: (id: string) => ["events", "detail", id] as const,
  place: (target: PlaceTarget) =>
    ["events", "place", "world" in target ? `world:${target.world}` : `position:${target.position.join(",")}`] as const,
};

export type LiveNowData = { events: Event[]; places: Place[] };

/** Live events with their connected players, and the busiest places. Either read failing fails the rail. */
export async function fetchLiveNow(opts: GetOptions = {}): Promise<LiveNowData> {
  const [events, places] = await Promise.all([
    fetchEvents({ list: "live", limit: LIVE_EVENTS_LIMIT, with_connected_users: true }, opts),
    fetchMostActivePlaces({ limit: HOT_PLACES_LIMIT }, opts),
  ]);
  return { events: events.data.filter(isPublic), places };
}

/** Events starting within the next day, not yet live. */
export async function fetchUpcoming(opts: GetOptions = {}, now = Date.now()): Promise<Event[]> {
  const { data } = await fetchEvents(
    {
      list: "upcoming",
      limit: UPCOMING_LIMIT,
      from: new Date(now).toISOString(),
      to: new Date(now + UPCOMING_WINDOW_MS).toISOString(),
    },
    opts,
  );
  return data.filter((event) => isPublic(event) && !event.live);
}

export async function fetchCalendar(opts: GetOptions = {}): Promise<Event[]> {
  const { data } = await fetchEvents({ list: "active", limit: CALENDAR_LIMIT }, opts);
  return data.filter(isPublic);
}

/** Null when the events service does not know the id; an outage throws. */
export async function fetchEventDetail(id: string, opts: GetOptions = {}): Promise<Event | null> {
  try {
    const event = await fetchEvent(id, opts);
    return isPublic(event) ? event : null;
  } catch (error) {
    if (error instanceof CatalystError && (error.status === 400 || error.status === 404)) return null;
    throw error;
  }
}

export async function fetchPlaceDetail(target: PlaceTarget, opts: GetOptions = {}): Promise<JumpPlaceRecord | null> {
  return "world" in target
    ? loadJumpPlace({ lookup: undefined, realm: target.world }, opts)
    : loadJumpPlace({ lookup: target.position, realm: undefined }, opts);
}

/** The events API already leaves out unapproved and deleted events; an open modal must not show one either. */
function isPublic(event: Event): boolean {
  if (event.approved === false || event.rejected === true) return false;
  return !event.deleted_by_user && !event.deleted_by_admin;
}

/**
 * The place a page URL names. A bare World name (`brai`) is the `.dcl.eth` name, as older share links
 * carry it; a World wins over a position. Null for a missing or malformed target.
 */
export function readPlaceTarget(params: URLSearchParams): PlaceTarget | null {
  const world = params.get("world")?.trim().toLowerCase();
  if (world) {
    const name = world.endsWith(".eth") ? world : `${world}.dcl.eth`;
    if (isEnsRealm(name)) return { world: name };
  }
  const position = params.get("position")?.trim();
  if (position) {
    const parsed = parsePosition(position);
    if (parsed.isValid) return { position: parsed.coordinates };
  }
  return null;
}

/** An event id link: any non-empty value; the events service decides whether it exists. */
export function readEventId(params: URLSearchParams): string | null {
  return params.get("id")?.trim() || null;
}

export const liveNowQuery = () =>
  queryOptions({
    queryKey: eventsPageKeys.live(),
    staleTime: EVENTS_PAGE_STALE_MS,
    queryFn: ({ signal }) => fetchLiveNow({ signal }),
  });

export const upcomingQuery = () =>
  queryOptions({
    queryKey: eventsPageKeys.upcoming(),
    staleTime: EVENTS_PAGE_STALE_MS,
    queryFn: ({ signal }) => fetchUpcoming({ signal }),
  });

export const calendarQuery = () =>
  queryOptions({
    queryKey: eventsPageKeys.calendar(),
    staleTime: EVENTS_PAGE_STALE_MS,
    queryFn: ({ signal }) => fetchCalendar({ signal }),
  });

export const eventDetailQuery = (id: string) =>
  queryOptions({
    queryKey: eventsPageKeys.detail(id),
    staleTime: EVENTS_PAGE_STALE_MS,
    queryFn: ({ signal }) => fetchEventDetail(id, { signal }),
  });

export const placeDetailQuery = (target: PlaceTarget) =>
  queryOptions({
    queryKey: eventsPageKeys.place(target),
    staleTime: EVENTS_PAGE_STALE_MS,
    queryFn: ({ signal }) => fetchPlaceDetail(target, { signal }),
  });

/** What the page's first document holds for agents: the live and upcoming events it dehydrated. */
export function eventsAgentData(state: DehydratedState | null | undefined): { live: Event[]; upcoming: Event[] } {
  const read = <T>(key: readonly unknown[]): T | undefined =>
    state?.queries.find((query) => JSON.stringify(query.queryKey) === JSON.stringify(key))?.state.data as T | undefined;
  return {
    live: read<LiveNowData>(eventsPageKeys.live())?.events ?? [],
    upcoming: read<Event[]>(eventsPageKeys.upcoming()) ?? [],
  };
}

/** The event a `?id=` link opened, when the page's first document dehydrated it, for its agent markdown. */
export function eventsAgentDetail(state: DehydratedState | null | undefined): Event | null {
  const query = state?.queries.find((entry) => entry.queryKey[0] === "events" && entry.queryKey[1] === "detail");
  return (query?.state.data as Event | null | undefined) ?? null;
}
