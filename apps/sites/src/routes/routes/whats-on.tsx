import { useEffect, useRef } from "react";
import { Outlet, redirect, useRevalidator, useSearchParams } from "react-router";
import { href } from "@/core/lib/router/routes";
import { Button } from "@dcl/one-ui/components";

import StWhatSOn, { type WoDayEvent, type WoLiveCard, type WoUpcomingCard } from "@ui/web/pages/StWhatSOn";
import FreshnessNotice from "@ui/components/FreshnessNotice";
import { olderCopy, type CopyStamp } from "@ui/data/copyAsOf";
import { asset } from "@ui/asset";
import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";

import {
  bucketEventsByDay,
  dayHeaderLabel,
  EVENT_CALENDAR_LIMIT,
  fetchEvents,
  hueFor,
  isPublicEvent,
  toUpcomingCard,
  type DayEvent,
  type Event,
} from "@/data/lib/catalyst/places/events";
import { fetchEventCalendar } from "@/data/lib/catalyst/places/event-calendar.server";
import {
  DCL_FOUNDATION_NAME,
  formatEthAddress,
  loadLiveNowCards,
  loadProfiles,
  type LiveNowCard,
  type Profile,
} from "@/data/lib/catalyst/places/live-now.server";
import { playUrl } from "@/data/lib/catalyst/bases";
import { useAuth } from "@/data/lib/auth/context";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";
import EventManagement from "@/features/explorer/event-management/EventManagement";

import styles from "./whats-on.module.css";

import type { GetOptions } from "@/data/lib/catalyst/client";
import type { MouseEvent } from "react";
import type { Route } from "./+types/whats-on";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import type { StoryId } from "@/core/lib/telemetry/story-id";

export const handle = { agentMarkdown: "whatsOn" } satisfies AgentMarkdownHandle;

const STORY: StoryId = "admin/whats-on";
/** Upcoming: the next ten events that start within a day. */
const UPCOMING_LIMIT = 10;
const UPCOMING_WINDOW_MS = 86_400_000;
/** The calendar's days, today first; the date bar pages through them. */
const CALENDAR_DAYS = 28;
/** The day columns the calendar's first page can show (StWhatSOn MAX_COLUMNS): their creators' faces come with the page. */
const FACE_COLUMNS = 4;
const FOUNDATION_COLOR = "#9d76e3";
const FOUNDATION_LOGO = asset("assets/home/dcl-logo.svg");

const FALLBACK: PageDefaults = {
  variant: "live_feed",
};

const settled = <T extends object>(read: Promise<T>, empty: T) =>
  read.then((result) => ({ ...result, unavailable: false })).catch(() => ({ ...empty, unavailable: true }));

const isFoundation = (userName: string | null | undefined) => userName?.trim().toLowerCase() === DCL_FOUNDATION_NAME.toLowerCase();

/**
 * The creators' profiles, one request per card group as the reference's cards ask them (the live and upcoming
 * cards together, then each day column), skipping the Foundation's events, which show its logo.
 */
async function loadFaces(live: LiveNowCard[], upcoming: Event[], allDays: DayEvent[][], opts: GetOptions): Promise<Record<string, Profile>> {
  const creator = (user: string | null | undefined, userName: string | null | undefined) => (user && !isFoundation(userName) ? [user] : []);
  const groups = [
    [...live.flatMap((c) => (c.creatorAddress && !c.isGenesisPlaza ? [c.creatorAddress] : [])), ...upcoming.flatMap((e) => creator(e.user, e.user_name))],
    ...allDays.slice(0, FACE_COLUMNS).map((day) => day.flatMap((e) => creator(e.user, e.userName))),
  ];
  const read = await Promise.all(groups.map((ids) => (ids.length ? loadProfiles(ids, opts).catch(() => ({})) : {})));
  return Object.assign({}, ...read);
}

/** The reference's useCreatorProfile: the Foundation's logo, else the profile's name and face over its colour. */
function creatorOf(user: string | null | undefined, userName: string | null | undefined, fallback: string, profiles: Record<string, Profile>) {
  if (isFoundation(userName)) return { creator: DCL_FOUNDATION_NAME, face: FOUNDATION_LOGO, color: FOUNDATION_COLOR };
  const profile = user ? profiles[user.toLowerCase()] : undefined;
  const creator = profile?.name || userName || fallback;
  const color = profile?.name
    ? getAvatarBackgroundColor(getDisplayName(profile.name, profile.hasClaimedName, profile.address))
    : getAvatarBackgroundColor(getDisplayName(creator, false, user));
  return { creator, face: profile?.face ?? null, color };
}

/** The reference's LiveNowCardItem: the card's own name first, then the Foundation, then the address. */
function toLiveCard(card: LiveNowCard, profiles: Record<string, Profile>): WoLiveCard {
  const profile = card.creatorAddress ? profiles[card.creatorAddress.toLowerCase()] : undefined;
  const creator = card.creatorName ?? card.userName ?? (card.isGenesisPlaza ? DCL_FOUNDATION_NAME : card.creatorAddress ? formatEthAddress(card.creatorAddress) : "Unknown");
  return {
    id: card.id,
    title: card.title,
    users: card.users,
    isEvent: !card.place,
    creator,
    hue: hueFor(card.id),
    image: card.image || undefined,
    jumpUrl: playUrl({ position: card.coordinates }),
    face: card.isGenesisPlaza ? FOUNDATION_LOGO : profile?.face ?? null,
    color: card.isGenesisPlaza ? FOUNDATION_COLOR : getAvatarBackgroundColor(getDisplayName(profile?.name, profile?.hasClaimedName, profile?.address ?? card.creatorAddress)),
  };
}

export async function loader({ request, context }: Route.LoaderArgs) {
  const url = new URL(request.url);
  // The reference's event link, /events?id=<id>, opens that event's dialog: here it is the event's own page.
  const eventId = url.searchParams.get("id");
  if (eventId && url.pathname.replace(/\/$/, "") === href("/whats-on")) throw redirect(href("/whats-on/:id", { id: eventId }));
  if (url.searchParams.get("tab") === "my") {
    return { tab: "my" as const, sid: "", unavailable: false, liveUnavailable: false, upcomingUnavailable: false, calendarLimited: false, invalidCount: 0, live: [], upcoming: [], allDays: [], dayLabels: [], copy: { stamp: null, now: Date.now() } };
  }
  const now = new Date();
  const opts = { signal: request.signal };
  const empty = { data: [] as Event[], total: 0, copy: null };
  const [{ sid, wrap }, liveRes, upcomingRes, calendarRes] = await Promise.all([
    pageLoader(request, context, FALLBACK),
    settled(loadLiveNowCards(opts), { cards: [] as LiveNowCard[], copy: null }),
    settled(fetchEvents({ list: "upcoming", limit: UPCOMING_LIMIT, order: "asc", from: now.toISOString(), to: new Date(now.getTime() + UPCOMING_WINDOW_MS).toISOString() }, opts), empty),
    settled(fetchEventCalendar({ order: "asc" }, opts), empty),
  ]);
  const upcomingEvents = upcomingRes.data.filter(isPublicEvent);
  const days = bucketEventsByDay(calendarRes.data, CALENDAR_DAYS, now);
  const profiles = await loadFaces(liveRes.cards, upcomingEvents, days, opts);
  const live = liveRes.cards.map((card) => toLiveCard(card, profiles));
  const upcoming: WoUpcomingCard[] = upcomingEvents.map((event) => {
    const card = toUpcomingCard(event, now);
    return { ...card, attending: event.attending, ...creatorOf(event.user, event.user_name, card.creator, profiles) };
  });
  const allDays: WoDayEvent[][] = days.map((day) => day.map((event) => ({ ...event, ...creatorOf(event.user, event.userName, event.creator, profiles) })));
  // The calendar reads the first EVENT_CALENDAR_LIMIT active events, as the reference does; a full read may leave
  // later days short, which the page says.
  const calendarLimited = ("received" in calendarRes ? calendarRes.received ?? calendarRes.data.length : calendarRes.data.length) >= EVENT_CALENDAR_LIMIT;
  const dayLabels = Array.from({ length: CALENDAR_DAYS }, (_, offset) => dayHeaderLabel(offset, now));
  const copy = { stamp: olderCopy(olderCopy(liveRes.copy, upcomingRes.copy), calendarRes.copy), now: now.getTime() };
  const invalidCount = [upcomingRes, calendarRes].reduce((count, result) => count + ("invalidCount" in result ? result.invalidCount ?? 0 : 0), 0);
  const payload = {
    sid,
    unavailable: calendarRes.unavailable,
    liveUnavailable: liveRes.unavailable,
    upcomingUnavailable: upcomingRes.unavailable,
    calendarLimited,
    invalidCount,
    live,
    upcoming,
    allDays,
    dayLabels,
    copy,
  };

  return wrap(payload, { status: calendarRes.unavailable ? 503 : 200 });
}

export default function WhatsOn({ loaderData }: Route.ComponentProps) {
  const d = loaderData;
  const [searchParams] = useSearchParams();
  if (searchParams.get("tab") === "my") return <><EventManagement mode="mine" /><Outlet /></>;

  return (
    <>
    <WhatsOnFeed
      sid={d.sid}
      unavailable={d.unavailable}
      liveUnavailable={d.liveUnavailable}
      upcomingUnavailable={d.upcomingUnavailable}
      calendarLimited={d.calendarLimited}
      invalidCount={d.invalidCount}
      live={d.live}
      upcoming={d.upcoming}
      allDays={d.allDays}
      dayLabels={d.dayLabels}
      copy={d.copy}
    />
    <Outlet />
    </>
  );
}

type FeedProps = {
  sid: string;
  unavailable: boolean;
  liveUnavailable: boolean;
  upcomingUnavailable: boolean;
  /** The calendar read hit EVENT_CALENDAR_LIMIT: later days can miss events. */
  calendarLimited: boolean;
  invalidCount: number;
  live: WoLiveCard[];
  upcoming: WoUpcomingCard[];
  allDays: WoDayEvent[][];
  dayLabels: string[];
  /** The events copy's stamp, and the loader's clock. */
  copy: { stamp: CopyStamp | null; now: number };
};

function WhatsOnFeed({ sid, unavailable, liveUnavailable, upcomingUnavailable, calendarLimited, invalidCount, live, upcoming, allDays, dayLabels, copy }: FeedProps) {
  const revalidator = useRevalidator();
  const { identity } = useAuth();

  useWhatsOnView(sid, live.length, upcoming.length);

  const failed = unavailable || liveUnavailable || upcomingUnavailable;
  const notice = <>
    {failed || calendarLimited || invalidCount > 0 ? <div role="alert" className={styles.unavailable}>
      {unavailable && <p>Events are temporarily unavailable.</p>}
      {liveUnavailable && <p>Live events are temporarily unavailable.</p>}
      {upcomingUnavailable && <p>Upcoming events are temporarily unavailable.</p>}
      {calendarLimited && <p>The calendar shows the first {EVENT_CALENDAR_LIMIT} events. Later days can be incomplete.</p>}
      {invalidCount > 0 && <p>Some events could not be displayed.</p>}
      {failed && <Button variant="outline" size="sm" onClick={() => { void revalidator.revalidate(); }}>Retry</Button>}
    </div> : null}
    <FreshnessNotice copy={copy.stamp} now={copy.now} />
  </>;

  // The cards are plain links to the event page; one listener records which one was opened.
  const onClickCapture = (e: MouseEvent<HTMLDivElement>) => {
    const href = (e.target as Element).closest?.("a[href^='/whats-on/']")?.getAttribute("href");
    if (href) track("lp_event_card_clicked", { event_id: decodeURIComponent(href.slice("/whats-on/".length)) }, { sid, story: STORY });
  };

  return (
    <div onClickCapture={onClickCapture}>
      <StWhatSOn
        notice={notice}
        liveNow={live}
        upcoming={upcoming}
        allDays={allDays}
        dayLabels={dayLabels}
        signedIn={!!identity}
      />
    </div>
  );
}

function useWhatsOnView(sid: string, liveCount: number, upcomingCount: number) {
  const sent = useRef(false);
  useEffect(() => {
    if (sent.current) return;
    sent.current = true;
    track(
      "lp_whatson_viewed",
      { live_count: liveCount, upcoming_count: upcomingCount, filter: null, search: null },
      { sid, story: STORY },
    );
  }, [sid, liveCount, upcomingCount]);
}
