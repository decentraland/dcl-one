import { z } from "zod";

import { playUrl } from "../bases";
import { CatalystError, getJSON } from "../client";
import type { GetOptions } from "../client";
import { eventsApiPath } from "../typed";
import { apiOkOf, okDataTotalOf } from "../envelope";

import {
  EventAttendeeRecordSchema,
  EventCategoryRecordSchema,
  EventRecordSchema,
} from "../generated-schemas/events";
import { warnInvalid } from "../warn";
import { readCopy, type CopyStamp } from "@ui/data/copyAsOf";
import type { EventAttendeeRecord as RsEventAttendee } from "@catalyst-api/events/EventAttendeeRecord";
import type { ApiData as RsApiOk } from "@catalyst-api/places/ApiData";
import type { EventCategoryRecord as RsEventCategory } from "@catalyst-api/events/EventCategoryRecord";
import type { EventRecord as RsEvent } from "@catalyst-api/events/EventRecord";
import type { EventListData as RsEventListData } from "@catalyst-api/events/EventListData";

export type Event = {
  id: string;
  name: string | null;
  image: string | null;
  image_vertical: string | null;
  description: string | null;
  start_at: string | null;
  finish_at: string | null;
  next_start_at: string | null;
  next_finish_at?: string | null;
  duration?: number | null;
  all_day: boolean;
  x: number | null;
  y: number | null;
  position: number[] | null;
  coordinates: number[] | null;
  url: string | null;
  user_name: string | null;
  scene_name: string | null;
  estate_name: string | null;
  live: boolean;
  highlighted: boolean;
  trending: boolean;
  recurrent: boolean;
  recurrent_frequency: string | null;
  total_attendees: number | null;
  place_id: string | null;
  connected_addresses?: string[];
  user?: string | null;
  approved?: boolean;
  rejected?: boolean;
  deleted_by_user?: boolean;
  deleted_by_admin?: boolean;
  recurrent_dates?: string[];
  recurrent_interval?: number | null;
  recurrent_weekday_mask?: number | null;
  world?: boolean;
  server?: string | null;
  attending?: boolean;
  categories?: string[];
};

export type EventCategory = {
  name: string;
  active: boolean;
  i18n: { en: string | null };
};

export type EventAttendee = z.infer<typeof EventAttendeeRecordSchema>;

function parseEvent(raw: unknown): Event {
  const r = EventRecordSchema.safeParse(raw);
  if (r.success) return r.data;
  warnInvalid("Event", r.error.issues);
  throw new CatalystError("event payload failed validation", "events");
}

function parseEvents(raw: unknown[]): (Event | null)[] {
  const out: (Event | null)[] = [];
  for (const row of raw ?? []) {
    const r = EventRecordSchema.safeParse(row);
    if (r.success) out.push(r.data);
    else {
      warnInvalid("Event", r.error.issues);
      out.push(null);
    }
  }
  return out;
}

function toEventCategory(c: z.infer<typeof EventCategoryRecordSchema>): EventCategory {
  const en = c.i18n["en"];
  return {
    name: c.name,
    active: c.active,
    i18n: { en: typeof en === "string" ? en : null },
  };
}

export const EVENT_CALENDAR_LIMIT = 200;

type FetchEventsParams = {
  list?: "live" | "active" | "upcoming" | "highlight" | "trending";
  order?: "asc" | "desc";
  search?: string;
  category?: string;
  from?: string;
  to?: string;
  recurrent?: boolean;
  with_connected_users?: boolean;
  limit?: number;
  offset?: number;
  /** A parcel as "x,y". */
  position?: string;
  world_names?: string;
  /** False keeps World events out of a Genesis City parcel lookup. */
  world?: boolean;
};

const EventListEnvelope = apiOkOf(
  z.union([
    z.array(z.unknown()),
    z.object({ events: z.array(z.unknown()), total: z.number() }),
  ]),
);
const EventDetailEnvelope = apiOkOf(z.unknown());
const CategoryListEnvelope = apiOkOf(z.array(EventCategoryRecordSchema));
const AttendeeListEnvelope = okDataTotalOf(z.array(EventAttendeeRecordSchema));

export type _DriftEventListEnvelope = Assert<
  AssignableTo<RsApiOk<RsEventListData>, z.input<typeof EventListEnvelope>>
>;

export type _DriftEventDetailEnvelope = Assert<
  AssignableTo<RsApiOk<RsEvent>, z.input<typeof EventDetailEnvelope>>
>;

export type _DriftCategoryListEnvelope = Assert<
  AssignableTo<RsApiOk<RsEventCategory[]>, z.input<typeof CategoryListEnvelope>>
>;

export type _DriftAttendeeListEnvelope = Assert<
  AssignableTo<RsApiOk<RsEventAttendee[]>, z.input<typeof AttendeeListEnvelope>>
>;

async function fetchEventRows(
  params: FetchEventsParams,
  opts: GetOptions,
): Promise<{ rows: (Event | null)[]; total: number; copy: CopyStamp | null }> {
  // `/v1/events`: the strict `/api/events` schema (upstream's `additionalProperties: false`)
  // refuses `recurrent`, which this site sends; the v1 list keeps the same envelope.
  const raw = await getJSON<unknown>(eventsApiPath("get", "/v1/events"), {
    ...opts,
    query: {
      list: params.list,
      search: params.search,
      from: params.from,
      to: params.to,
      recurrent: params.recurrent,
      with_connected_users: params.with_connected_users,
      limit: params.limit,
      offset: params.offset,
      position: params.position,
      world_names: params.world_names,
      world: params.world,
      order: params.order,
    },
  });
  const copy = readCopy(raw);
  const env = EventListEnvelope.safeParse(raw);
  if (!env.success) {
    warnInvalid("Events envelope", env.error.issues);
    throw new CatalystError("events envelope failed validation", "events");
  }
  const payload = env.data.data;
  const rows = parseEvents(Array.isArray(payload) ? payload : payload.events);
  return { rows, total: Array.isArray(payload) ? rows.length : payload.total, copy };
}

export async function fetchEvents(
  params: FetchEventsParams = {},
  opts: GetOptions = {},
): Promise<{ data: Event[]; total: number; received?: number; invalidCount?: number; copy?: CopyStamp | null }> {
  const { rows, total, copy } = await fetchEventRows(params, opts);
  return { data: rows.filter((event) => event !== null), total, received: rows.length, invalidCount: rows.filter((event) => event === null).length, copy };
}

/** Slice server slots before discarding malformed records so lookahead never leaks into this page. */
export async function fetchEventPage(
  params: FetchEventsParams & { limit: number },
  opts: GetOptions = {},
): Promise<{ data: Event[]; hasMore: boolean; invalidCount?: number; copy?: CopyStamp | null }> {
  const { rows, copy } = await fetchEventRows({ ...params, limit: params.limit + 1 }, opts);
  return {
    data: rows.slice(0, params.limit).filter((event) => event !== null),
    hasMore: rows.length > params.limit,
    invalidCount: rows.slice(0, params.limit).filter((event) => event === null).length,
    copy,
  };
}

export async function fetchEvent(id: string, opts: GetOptions = {}): Promise<Event> {
  const raw = await getJSON<unknown>(
    eventsApiPath("get", "/api/events/{event_id}", { event_id: id }),
    opts,
  );
  const env = EventDetailEnvelope.safeParse(raw);
  if (env.success) return parseEvent(env.data.data);
  warnInvalid("Event envelope", env.error.issues);
  return parseEvent(raw);
}

const EVENT_CATEGORIES_TTL_MS = 5 * 60_000;
let eventCategoriesCache: { at: number; value: EventCategory[] } | null = null;

export async function fetchEventCategories(
  opts: GetOptions = {},
): Promise<EventCategory[]> {
  const cacheable = !opts.fetchImpl;
  if (
    cacheable &&
    eventCategoriesCache &&
    Date.now() - eventCategoriesCache.at < EVENT_CATEGORIES_TTL_MS
  ) {
    return eventCategoriesCache.value;
  }
  const raw = await getJSON<unknown>(
    eventsApiPath("get", "/api/events/categories"),
    opts,
  );
  const env = CategoryListEnvelope.safeParse(raw);
  if (!env.success) {
    warnInvalid("EventCategory envelope", env.error.issues);
    return [];
  }
  const value = env.data.data.map(toEventCategory);
  if (cacheable && value.length > 0) {
    eventCategoriesCache = { at: Date.now(), value };
  }
  return value;
}

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;

type Assert<T extends true> = T;

export async function fetchAttendees(
  id: string,
  opts: GetOptions = {},
): Promise<{ attendees: EventAttendee[]; count: number }> {
  const raw = await getJSON<unknown>(
    eventsApiPath("get", "/api/events/{event_id}/attendees", { event_id: id }),
    opts,
  );
  const env = AttendeeListEnvelope.safeParse(raw);
  if (!env.success) {
    warnInvalid("EventAttendee envelope", env.error.issues);
    return { attendees: [], count: 0 };
  }
  const attendees = env.data.data;
  return { attendees, count: env.data.total ?? attendees.length };
}

export function hueFor(id: string): number {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) % 360;
  return h;
}

export type LiveNowCard = {
  id: string;
  title: string;
  users: number | null;
  isEvent: boolean;
  creator: string;
  hue: number;
  image?: string;
  jumpUrl?: string;
};

type UpcomingCard = {
  id: string;
  name: string;
  creator: string;
  time: string;
  hue: number;
  image?: string;
  jumpUrl?: string;
};

export function effectiveStartAt(e: Event): string | null {
  return eventOccurrence(e).start ?? null;
}

export function eventJumpUrl(e: Event): string | null {
  const [x, y] = eventPosition(e);
  if (x === null || y === null) return null;
  return playUrl({ position: `${x},${y}` });
}

function eventPosition(e: Event): [number | null, number | null] {
  return [e.x ?? e.position?.[0] ?? null, e.y ?? e.position?.[1] ?? null];
}

export function formatEventTime(iso: string | null): string {
  if (!iso) return "Soon";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "Soon";
  return d.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "UTC",
    hour12: false,
  });
}

export function formatEventWhen(iso: string | null): string {
  if (!iso) return "Date to be announced";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "Date to be announced";
  return d
    .toLocaleString("en-US", {
      weekday: "short",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      timeZone: "UTC",
      hour12: true,
    })
    .toUpperCase();
}

export function eventCoords(e: Event): string | null {
  const [x, y] = eventPosition(e);
  return x === null || y === null ? null : `(${x},${y})`;
}

export function toLiveNowCard(e: Event): LiveNowCard {
  return {
    id: e.id,
    title: e.name ?? "Untitled event",
    users: e.connected_addresses ? new Set(e.connected_addresses.map((address) => address.toLowerCase())).size : null,
    isEvent: true,
    creator: e.user_name ?? e.scene_name ?? "Decentraland",
    hue: hueFor(e.id),
    image: e.image ?? undefined,
    jumpUrl: eventJumpUrl(e) ?? undefined,
  };
}

export function toUpcomingCard(e: Event, now = new Date()): UpcomingCard {
  return {
    id: e.id,
    name: e.name ?? "Untitled event",
    creator: e.user_name ?? e.scene_name ?? "Decentraland",
    time: relativeTimeLabel(e.next_start_at ?? e.start_at, now),
    hue: hueFor(e.id),
    image: e.image ?? undefined,
    jumpUrl: eventJumpUrl(e) ?? undefined,
  };
}

export type DayEvent = {
  id: string;
  name: string;
  creator: string;
  /** The creator's address and `user_name`, for the face and name a profile read supplies. */
  user?: string;
  userName?: string;
  attending?: boolean;
  time: string;
  upcomingTime?: string;
  live: boolean;
  users?: number;
  x?: number;
  y?: number;
  hue: number;
  image?: string | null;
};

const DAY_MS = 86_400_000;

export function groupEventsByDay(
  events: Event[],
  liveIds: Set<string>,
  days = 7,
  now = new Date(),
): { allDays: DayEvent[][]; dayLabels: string[] } {
  const dayStart = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
  const allDays: DayEvent[][] = Array.from({ length: days }, () => []);
  for (const e of events) {
    const iso = effectiveStartAt(e);
    if (!iso) continue;
    const t = new Date(iso).getTime();
    if (Number.isNaN(t)) continue;
    const idx = Math.floor((t - dayStart) / DAY_MS);
    if (idx < 0 || idx >= days) continue;
    allDays[idx].push({
      id: e.id,
      name: e.name ?? "Untitled event",
      creator: e.user_name ?? e.scene_name ?? "Decentraland",
      time: new Date(iso).toLocaleTimeString("en-US", {
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
        timeZone: "UTC",
      }),
      live: liveIds.has(e.id),
      users: toLiveNowCard(e).users ?? undefined,
      x: e.x ?? e.position?.[0] ?? undefined,
      y: e.y ?? e.position?.[1] ?? undefined,
      hue: hueFor(e.id),
      image: e.image ?? undefined,
    });
  }
  for (const day of allDays) day.sort((a, b) => a.time.localeCompare(b.time));
  const dayLabels = Array.from({ length: days }, (_, i) => {
    if (i === 0) return "Today";
    if (i === 1) return "Tomorrow";
    return new Date(dayStart + i * DAY_MS)
      .toLocaleDateString("en-US", { weekday: "short", day: "numeric", timeZone: "UTC" })
      .toUpperCase();
  });
  return { allDays, dayLabels };
}

/** Public cards, detail pages and calendar exports refer to the same next occurrence. */
export function eventOccurrence(event: Pick<Event, "start_at" | "next_start_at" | "finish_at" | "next_finish_at" | "duration" | "recurrent">): { start: string | undefined; finish: string | undefined } {
  const start = event.next_start_at ?? event.start_at ?? undefined;
  const startTime = start ? Date.parse(start) : NaN;
  const duration = event.duration ?? (!event.recurrent && event.finish_at && event.start_at
    ? Date.parse(event.finish_at) - Date.parse(event.start_at)
    : undefined);
  const finish = event.next_finish_at && Date.parse(event.next_finish_at) > startTime
    ? event.next_finish_at
    : (Number.isFinite(startTime) && duration !== undefined && Number.isFinite(duration) && duration > 0
    ? new Date(startTime + duration).toISOString()
    : undefined);
  return { start, finish };
}

const utcDay = (t: number) => Math.floor(t / DAY_MS);

/** The clock time of an event in the page's time zone (UTC), "6:00 PM". */
export function formatEventClock(iso: string | null | undefined): string {
  const t = iso ? Date.parse(iso) : NaN;
  if (Number.isNaN(t)) return "Soon";
  return new Intl.DateTimeFormat("en-US", { hour: "numeric", minute: "2-digit", hour12: true, timeZone: "UTC" }).format(t);
}

/** "Starts in 12 mins", "Today 6:00 PM", "Tomorrow 1:00 AM" or the clock time. */
export function relativeTimeLabel(iso: string | null | undefined, now = new Date()): string {
  const t = iso ? Date.parse(iso) : NaN;
  if (Number.isNaN(t)) return "Soon";
  const diff = t - now.getTime();
  if (diff <= 0) return formatEventClock(iso);
  const mins = Math.round(diff / 60_000);
  if (mins < 60) return `Starts in ${mins} mins`;
  const days = utcDay(t) - utcDay(now.getTime());
  if (days === 0) return `Today ${formatEventClock(iso)}`;
  if (days === 1) return `Tomorrow ${formatEventClock(iso)}`;
  return formatEventClock(iso);
}

/** Only approved events that nobody rejected or deleted are listed in public. */
export function isPublicEvent(e: Event): boolean {
  return e.approved !== false && !e.rejected && !e.deleted_by_user && !e.deleted_by_admin;
}

/** The day header of the calendar column `offset` days after `now`'s day: "Today", "Tomorrow", "Wed, Nov 11". */
export function dayHeaderLabel(offset: number, now = new Date()): string {
  if (offset === 0) return "Today";
  if (offset === 1) return "Tomorrow";
  const day = new Date((utcDay(now.getTime()) + offset) * DAY_MS);
  const weekday = day.toLocaleDateString("en-US", { weekday: "short", timeZone: "UTC" });
  const month = day.toLocaleDateString("en-US", { month: "short", timeZone: "UTC" });
  return `${weekday}, ${month} ${day.getUTCDate()}`;
}

/**
 * One list per day from `now`'s day: each occurrence of a recurring event on its day, every other
 * event on the day it starts (today when it started earlier and still runs, as an event live across
 * midnight), without occurrences that already ended, sorted by start.
 */
export function bucketEventsByDay(events: Event[], days: number, now = new Date()): DayEvent[][] {
  const today = utcDay(now.getTime());
  const buckets: [number, DayEvent][][] = Array.from({ length: days }, () => []);
  for (const e of events) {
    if (!isPublicEvent(e)) continue;
    const recurring = e.recurrent && (e.recurrent_dates?.length ?? 0) > 0;
    for (const iso of recurring ? e.recurrent_dates! : [e.start_at]) {
      const start = iso ? Date.parse(iso) : NaN;
      if (Number.isNaN(start)) continue;
      const finish = recurring ? start + (e.duration ?? 0) : Date.parse(e.finish_at ?? "");
      if (finish < now.getTime()) continue;
      const startDay = utcDay(start) - today;
      const index = !recurring && startDay < 0 && !Number.isNaN(finish) ? 0 : startDay;
      if (index < 0 || index >= days) continue;
      const live = recurring ? start <= now.getTime() && now.getTime() <= finish : e.live;
      const at = e.next_start_at ?? new Date(start).toISOString();
      buckets[index].push([start, {
        id: e.id,
        name: e.name ?? "Untitled event",
        creator: e.user_name ?? e.scene_name ?? "Decentraland",
        user: e.user ?? undefined,
        userName: e.user_name ?? undefined,
        attending: e.attending,
        time: formatEventClock(at),
        upcomingTime: relativeTimeLabel(at, now),
        live,
        users: e.total_attendees ?? undefined,
        x: e.x ?? e.position?.[0] ?? undefined,
        y: e.y ?? e.position?.[1] ?? undefined,
        hue: hueFor(e.id),
        image: e.image ?? undefined,
      }]);
    }
  }
  return buckets.map((bucket) => bucket.sort((a, b) => a[0] - b[0]).map(([, event]) => event));
}
