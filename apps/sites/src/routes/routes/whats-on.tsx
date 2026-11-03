import { useEffect, useRef } from "react";
import { Form, Link, useNavigate, useNavigation, useRevalidator, useSearchParams } from "react-router";

import LdWhatsOnPage, { type LdWhatsOnEventLink } from "@ui/landings/pages/LdWhatsOnPage";
import FreshnessNotice from "@ui/components/FreshnessNotice";
import { olderCopy, type CopyStamp } from "@ui/data/copyAsOf";

import {
  fetchEvents,
  fetchEventPage,
  groupEventsByDay,
  toLiveNowCard,
  toUpcomingCard,
  type DayEvent,
  type Event,
} from "@/data/lib/catalyst/places/events";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";
import EventManagement, { EventDiscoveryNavigation } from "@/features/explorer/event-management/EventManagement";

import styles from "./whats-on.module.css";

import type { Route } from "./+types/whats-on";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import type { StoryId } from "@/core/lib/telemetry/story-id";

export const handle = { agentMarkdown: "whatsOn" } satisfies AgentMarkdownHandle;

const STORY: StoryId = "admin/whats-on";
const LIVE_LIMIT = 8;
const UPCOMING_LIMIT = 24;
const CALENDAR_LIMIT = 500;


const FILTERS = [
  { id: "", label: "All" },
  { id: "today", label: "Today" },
  { id: "week", label: "This week" },
  { id: "recurring", label: "Recurring" },
] as const;
type FilterId = (typeof FILTERS)[number]["id"];

const FALLBACK: PageDefaults = {
  variant: "live_feed",
};

export async function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  if (url.searchParams.get("tab") === "my") {
    return { tab: "my" as const, sid: "", offset: 0, hasMore: false, unavailable: false, liveUnavailable: false, calendarUnavailable: false, calendarLimited: false, invalidCount: 0, filter: "" as FilterId, search: "", live: [], upcoming: [], allDays: [], dayLabels: [], copy: { stamp: null, now: Date.now() } };
  }
  const rawFilter = url.searchParams.get("filter")?.trim() ?? "";
  const filter: FilterId = (FILTERS.some((f) => f.id === rawFilter) ? rawFilter : "") as FilterId;
  const search = url.searchParams.get("search")?.trim() ?? "";

  const parsedOffset = Number(url.searchParams.get("offset") ?? 0);
  const offset = Number.isSafeInteger(parsedOffset) && parsedOffset >= 0 ? parsedOffset : 0;
  const now = new Date();
  const dayStart = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
  const dateFiltered = filter === "today" || filter === "week";
  const filters = {
    search: search || undefined,
    from: dateFiltered ? new Date(dayStart).toISOString() : undefined,
    to: dateFiltered ? new Date(dayStart + (filter === "today" ? 1 : 7) * 86_400_000).toISOString() : undefined,
    recurrent: filter === "recurring" ? true : undefined,
  };
  const [{ sid, wrap }, liveRes, activeRes, calendarRes] = await Promise.all([
    pageLoader(request, FALLBACK),
    fetchEvents({ search: filters.search, recurrent: filters.recurrent, list: "live", limit: LIVE_LIMIT, with_connected_users: true }, { signal: request.signal })
      .then((result) => ({ ...result, unavailable: false }))
      .catch(() => ({ data: [] as Event[], copy: null, unavailable: true })),
    fetchEventPage({ ...filters, list: "active", limit: UPCOMING_LIMIT, offset }, { signal: request.signal })
      .then((result) => ({ ...result, unavailable: false }))
      .catch(() => ({ data: [] as Event[], hasMore: false, copy: null, unavailable: true })),
    fetchEvents({ ...filters, list: "active", from: filters.from ?? new Date(dayStart).toISOString(), to: filters.to ?? new Date(dayStart + 28 * 86_400_000).toISOString(), limit: CALENDAR_LIMIT }, { signal: request.signal })
      .then((result) => ({ ...result, unavailable: false }))
      .catch(() => ({ data: [] as Event[], copy: null, unavailable: true })),
  ]);
  const live = liveRes.data;
  const hasMore = activeRes.hasMore;
  const upcoming = activeRes.data;
  const copy = { stamp: olderCopy(olderCopy(liveRes.copy, activeRes.copy), calendarRes.copy), now: now.getTime() };
  const liveIds = new Set(live.map((event) => event.id));
  const liveById = new Map(live.map((event) => [event.id, event]));
  const { allDays, dayLabels } = groupEventsByDay(calendarRes.data.map((event) => liveById.get(event.id) ?? event), liveIds, 28, now);
  const unavailable = activeRes.unavailable;
  const liveUnavailable = liveRes.unavailable;
  const calendarUnavailable = calendarRes.unavailable;
  const calendarLimited = ("received" in calendarRes ? calendarRes.received : calendarRes.data.length) === CALENDAR_LIMIT;
  const invalidCount = [activeRes, liveRes, calendarRes].reduce((count, result) => count + ("invalidCount" in result ? result.invalidCount ?? 0 : 0), 0);
  const payload = {
    sid,
    offset,
    hasMore,
    unavailable,
    liveUnavailable,
    calendarUnavailable,
    calendarLimited,
    invalidCount,
    filter,
    search,
    live,
    upcoming,
    allDays,
    dayLabels,
    copy,
  };

  return wrap(payload, { status: unavailable ? 503 : 200 });
}

export default function WhatsOn({ loaderData }: Route.ComponentProps) {
  const d = loaderData;
  const [searchParams] = useSearchParams();
  if (searchParams.get("tab") === "my") return <EventManagement mode="mine" />;

  return (
    <WhatsOnFeed
      sid={d.sid}
      offset={d.offset}
      hasMore={d.hasMore}
      unavailable={d.unavailable}
      liveUnavailable={d.liveUnavailable}
      calendarUnavailable={d.calendarUnavailable}
      calendarLimited={d.calendarLimited}
      invalidCount={d.invalidCount}
      filter={d.filter}
      search={d.search}
      live={d.live}
      upcoming={d.upcoming}
      allDays={d.allDays}
      dayLabels={d.dayLabels}
      copy={d.copy}
    />
  );
}

type FeedProps = {
  sid: string;
  offset: number;
  hasMore: boolean;
  unavailable: boolean;
  liveUnavailable: boolean;
  calendarUnavailable: boolean;
  calendarLimited: boolean;
  invalidCount: number;
  filter: FilterId;
  search: string;
  live: Event[];
  upcoming: Event[];
  allDays: DayEvent[][];
  dayLabels: string[];
  /** The events copy's stamp, and the loader's clock. */
  copy: { stamp: CopyStamp | null; now: number };
};

function WhatsOnFeed({
  sid,
  offset,
  hasMore,
  unavailable,
  liveUnavailable,
  calendarUnavailable,
  calendarLimited,
  invalidCount,
  filter,
  search,
  live,
  upcoming,
  allDays,
  dayLabels,
  copy,
}: FeedProps) {
  const [, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const navigation = useNavigation();
  const revalidator = useRevalidator();
  const [params] = useSearchParams();
  const pageHref = (nextOffset: number) => {
    const next = new URLSearchParams(params);
    if (nextOffset) next.set("offset", String(nextOffset));
    else next.delete("offset");
    return `/whats-on?${next.toString()}`;
  };

  useWhatsOnView(sid, live.length, upcoming.length, filter, search);

  const liveCards = live.map(toLiveNowCard);
  const upcomingCards = upcoming.map(toUpcomingCard);

  const eventLinks: LdWhatsOnEventLink[] = dedupeById([...live, ...upcoming]).map((e) => ({
    id: e.id,
    href: `/whats-on/${encodeURIComponent(e.id)}`,
    name: e.name ?? "Untitled event",
    live: e.live,
  }));

  function updateParam(key: string, value: string) {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete("offset");
        if (value) next.set(key, value);
        else next.delete(key);
        return next;
      },
      { preventScrollReset: true },
    );
  }

  return (
    <LdWhatsOnPage
      navigationSlot={<>
        <EventDiscoveryNavigation />
        <Form method="get" role="search" className={styles.search}>
          {filter && <input type="hidden" name="filter" value={filter} />}
          <label htmlFor="hangout-search">Search hangouts</label>
          <input key={search} id="hangout-search" name="search" type="search" defaultValue={search} maxLength={150} />
          <button type="submit" disabled={navigation.state !== "idle"}>Search</button>
          {search && <Link to={filter ? `/whats-on?filter=${filter}` : "/whats-on"}>Clear search</Link>}
        </Form>
      </>}
      statusSlot={unavailable || liveUnavailable || calendarUnavailable || calendarLimited || invalidCount > 0 ? <div role="alert" className={styles.unavailable}>
        {unavailable && <p>Hangouts are temporarily unavailable.</p>}
        {liveUnavailable && <p>Live hangouts are temporarily unavailable.</p>}
        {calendarUnavailable && <p>The hangout calendar is temporarily unavailable.</p>}
        {calendarLimited && <p>The calendar shows the first 500 matching hangouts. Narrow your search to see more.</p>}
        {invalidCount > 0 && <p>Some hangouts could not be displayed.</p>}
        {(unavailable || liveUnavailable || calendarUnavailable || invalidCount > 0) && <button onClick={() => { void revalidator.revalidate(); }}>Retry</button>}
      </div> : undefined}
      liveNow={liveCards}
      upcoming={upcomingCards}
      allDays={allDays}
      dayLabels={dayLabels}
      filters={[...FILTERS]}
      activeFilter={filter}
      onFilterSelect={(id) => updateParam("filter", id)}
      eventLinks={eventLinks}
      onEventLinkClick={(link, e) => {
        e.preventDefault();
        track("lp_event_card_clicked", { event_id: link.id }, { sid, story: STORY });
        navigate(link.href);
      }}
      promoSlot={
        <>
          <FreshnessNotice copy={copy.stamp} now={copy.now} />
          {!unavailable && <nav className={styles.pager} aria-label="Event pages">
            {offset > 0 && <Link to={pageHref(Math.max(0, offset - UPCOMING_LIMIT))}>Previous page</Link>}
            <span>Page {Math.floor(offset / UPCOMING_LIMIT) + 1}</span>
            {hasMore && <Link to={pageHref(offset + UPCOMING_LIMIT)}>Next page</Link>}
          </nav>}
        </>
      }
    />
  );
}

function dedupeById(events: Event[]): Event[] {
  const seen = new Set<string>();
  const out: Event[] = [];
  for (const e of events) {
    if (seen.has(e.id)) continue;
    seen.add(e.id);
    out.push(e);
  }
  return out;
}

function useWhatsOnView(
  sid: string,
  liveCount: number,
  upcomingCount: number,
  filter: string,
  search: string,
) {
  const lastKey = useRef<string | null>(null);
  useEffect(() => {
    const key = `${filter}|${search}`;
    if (lastKey.current === key) return;
    lastKey.current = key;
    track(
      "lp_whatson_viewed",
      {
        live_count: liveCount,
        upcoming_count: upcomingCount,
        filter: filter || null,
        search: search || null,
      },
      { sid, story: STORY },
    );
  }, [sid, liveCount, upcomingCount, filter, search]);
}
