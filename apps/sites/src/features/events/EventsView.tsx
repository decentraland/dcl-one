import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useQuery } from "@tanstack/react-query";

import {
  AllEvents,
  type AllEventsDay,
  PlaceDetailModal,
  EventsPage,
  HostBanner,
  EventsLiveNow as LiveNow,
  Upcoming,
  type ExperienceCardProps,
  type JumpIn,
  type LiveEventCardProps,
  type UpcomingEventCardProps,
  useEventsCopy,
} from "@dcl/one-ui";

import { useAuth } from "@/data/lib/auth/context";
import { siteUrl } from "@/data/lib/catalyst/bases";
import type { JumpPlaceRecord } from "@/data/lib/catalyst/jump/jump";
import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  calendarQuery,
  DEFAULT_MIN_USERS,
  eventDetailQuery,
  liveNowQuery,
  LIVE_NOW_REFRESH_MS,
  placeDetailQuery,
  readEventId,
  readPlaceTarget,
  upcomingQuery,
} from "@/data/lib/catalyst/places/events-page";
import { track } from "@/core/lib/telemetry/track";
import type { StoryId } from "@/core/lib/telemetry/story-id";
import { openSignIn } from "../components/auth/signin-store";
import { useSignInRequired } from "../components/auth/useSignInRequired";
import type { DetectedPlatform } from "../download/options";
import { EventDetail, eventJumpIn, isPlainClick, remindControl } from "./EventDetail";
import { useCreatorProfiles } from "./useCreatorProfiles";
import { REMIND_FAILED, useEventReminders } from "./useEventReminders";
import { myEventCards, myEventsQuery } from "./my-events";
import type { CreatorProfiles } from "@/data/lib/catalyst/social/creator-profiles";
import { formatTime, type Zone } from "../components/event-schedule";
import {
  bucketByDay,
  creatorWallets,
  calendarHref,
  closedHref,
  DAY_PARAM,
  EVENTS_PATH,
  MAX_DAY_OFFSET,
  dayFullLabel,
  dayLabel,
  eventCreator,
  eventHref,
  eventImage,
  eventPageShareUrl,
  eventTarget,
  eventUsers,
  liveNowItems,
  upcomingCardProps,
  locationLabel,
  placeCreator,
  placeHref,
  placeShareUrl,
  placeTarget,
  readDayOffset,
  relativeStart,
  TAB_PARAM,
  usersLabel,
  visibleDays,
  type Target,
} from "./view";

const STORY: StoryId = "admin/whats-on";
const LEARN_MORE_PATH = "/blog/about-decentraland/how-to-make-and-run-an-event-in-decentraland";
const CREATE_HREF = "/events/new-event";
/** The All Events section, which arriving at My Events scrolls to. */
const ALL_EVENTS_ID = "all-events";

export type EventsViewProps = {
  sid: string;
  platform: DetectedPlatform;
  /** The public origin share links and calendar entries point at. */
  origin: string;
  /** The origin the browser loaded the page from: the request's on the server, the window's in the browser. */
  pageOrigin: string;
  /** The loader's clock and its required-data outcome; null after a client navigation. */
  initial: { now: number; unavailable: boolean } | null;
};

/** Day columns: five on wide screens (source's 1536px), four from 1024px, two from 768px, a list below. */
export function columnsFor(width: number): number {
  if (width >= 1536) return 5;
  if (width >= 1024) return 4;
  if (width >= 768) return 2;
  return 1;
}

/** The server renders UTC at its own clock and a width guess; the page moves to the viewer's after hydration. */
function useViewer(initialNow: number, platform: DetectedPlatform) {
  const [zone, setZone] = useState<Zone>("UTC");
  const [now, setNow] = useState(initialNow);
  const [columns, setColumns] = useState(platform.kind === "mobile" ? 1 : 4);
  useEffect(() => {
    setZone(undefined);
    setNow(Date.now());
    const update = () => setColumns(columnsFor(window.innerWidth));
    update();
    window.addEventListener("resize", update);
    const tick = setInterval(() => setNow(Date.now()), 60_000);
    return () => {
      window.removeEventListener("resize", update);
      clearInterval(tick);
    };
  }, []);
  return { zone, now, columns };
}

export function EventsView({ sid, platform, origin, pageOrigin, initial }: EventsViewProps) {
  const { locale, text } = useEventsCopy();
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const search = params.toString();
  const { identity } = useAuth();
  const { zone, now, columns } = useViewer(initial?.now ?? Date.now(), platform);

  const live = useQuery({ ...liveNowQuery(), refetchInterval: LIVE_NOW_REFRESH_MS });
  const upcoming = useQuery(upcomingQuery());
  const calendar = useQuery(calendarQuery());

  const eventId = readEventId(params);
  const placeTargetParam = eventId ? null : readPlaceTarget(params);
  const detail = useQuery({ ...eventDetailQuery(eventId ?? ""), enabled: Boolean(eventId) });
  const place = useQuery({
    ...placeDetailQuery(placeTargetParam ?? { position: [0, 0] }),
    enabled: Boolean(placeTargetParam),
  });

  const jumpIn = useCallback((target: Target): JumpIn => eventJumpIn(target, platform), [platform]);

  const open = useCallback(
    (href: string, eventIdForTelemetry?: string) => (event: MouseEvent<HTMLAnchorElement>) => {
      if (!isPlainClick(event)) return;
      event.preventDefault();
      if (eventIdForTelemetry && sid)
        track("lp_event_card_clicked", { event_id: eventIdForTelemetry }, { sid, story: STORY });
      void navigate(href, { preventScrollReset: true });
    },
    [navigate, sid],
  );

  const close = useCallback(() => {
    void navigate(closedHref(search), { replace: true, preventScrollReset: true });
  }, [navigate, search]);

  // A link to an event or place the catalogue does not know closes, as source; an outage keeps the URL.
  const missing =
    (eventId && detail.isSuccess && detail.data === null) ||
    (placeTargetParam && place.isSuccess && place.data === null);
  useEffect(() => {
    if (missing) close();
  }, [missing, close]);

  // All Events: the visible days and their occurrences.
  const offset = readDayOffset(params);
  const days = useMemo(() => visibleDays(now, zone, offset, columns), [now, zone, offset, columns]);
  const buckets = useMemo(
    () =>
      bucketByDay(
        calendar.data ?? [],
        days.map((day) => day.key),
        now,
        zone,
      ),
    [calendar.data, days, now, zone],
  );

  const upcomingEvents = useMemo(() => upcoming.data ?? [], [upcoming.data]);
  const mine = useMyEventsTab(identity);
  const reminders = useEventReminders(identity, () => openSignIn());

  const notice = reminders.failed ? text(REMIND_FAILED) : undefined;

  const minUsers = Number(params.get("minUsers"));
  const liveItems = useMemo(
    () =>
      live.data
        ? liveNowItems(live.data, Number.isFinite(minUsers) && params.has("minUsers") ? minUsers : DEFAULT_MIN_USERS)
        : [],
    [live.data, minUsers, params],
  );
  // One batched profile read per section, each asking only for wallets an earlier section did not.
  const creatorGroups = useMemo(
    () => [
      creatorWallets(liveItems.map((item) => (item.kind === "event" ? item.event : item.place))),
      creatorWallets(upcomingEvents),
      creatorWallets(buckets.flat().map((occurrence) => occurrence.event)),
      creatorWallets(detail.data ? [detail.data] : place.data ? [place.data] : []),
      creatorWallets(mine.events ?? []),
    ],
    [liveItems, upcomingEvents, buckets, detail.data, place.data, mine.events],
  );
  const profiles = useCreatorProfiles(creatorGroups);
  // Counted once both reads settled, so a client navigation never reports the empty first render.
  // Genesis Plaza shown because nothing is live is not a live item.
  const liveCount = liveItems.filter((item) => !(item.kind === "place" && item.fallback)).length;
  useViewedOnce(sid, !live.isPending && !upcoming.isPending, liveCount, upcomingEvents.length, offset);
  const liveCards: LiveEventCardProps[] = liveItems.map((item): LiveEventCardProps => {
    if (item.kind === "event") {
      const href = eventHref(item.event.id, search);
      return {
        id: item.event.id,
        title: item.event.name ?? text("Untitled event"),
        image: eventImage(item.event.image, pageOrigin),
        href,
        onOpen: open(href, item.event.id),
        users: item.users,
        usersLabel: item.users === undefined ? undefined : usersLabel(item.users, locale),
        creator: eventCreator(item.event, profiles, locale),
        live: true,
        jumpIn: jumpIn(eventTarget(item.event)),
      };
    }
    const target = placeTarget(item.place);
    const href = placeHref(target, search);
    return {
      id: item.place.id,
      title: item.place.title ?? text("Untitled place"),
      image: item.place.image,
      href,
      onOpen: open(href),
      users: item.users,
      usersLabel: usersLabel(item.users, locale),
      creator: placeCreator(item.place, profiles, locale),
      live: false,
      jumpIn: jumpIn(target),
    };
  });

  const upcomingCards: UpcomingEventCardProps[] = upcomingEvents.map((event) => {
    const href = eventHref(event.id, search);
    return {
      ...upcomingCardProps(event, { href, now, zone, origin, pageOrigin, profiles, locale }),
      onOpen: open(href, event.id),
      remind: remindControl(reminders.get(event.id)),
    };
  });

  const allDays: AllEventsDay[] = days.map((day, i) => ({
    key: day.key,
    label: dayLabel(day.date, offset + i, locale),
    fullLabel: dayFullLabel(day.date, locale),
    isToday: offset + i === 0,
    cards: (buckets[i] ?? []).map((occurrence): ExperienceCardProps => {
      const { event } = occurrence;
      const href = eventHref(event.id, search);
      const users = occurrence.live ? eventUsers(event) : undefined;
      return {
        id: `${event.id}@${occurrence.start}`,
        title: event.name ?? text("Untitled event"),
        image: eventImage(event.image, pageOrigin),
        href,
        onOpen: open(href, event.id),
        users,
        usersLabel: users === undefined ? undefined : usersLabel(users, locale),
        creator: eventCreator(event, profiles, locale),
        live: occurrence.live,
        time: {
          label: columns === 1 ? relativeStart(occurrence.start, now, zone, locale) : formatTime(occurrence.start, zone, locale),
          dateTime: occurrence.start,
        },
        remind: occurrence.live ? undefined : remindControl(reminders.get(event.id)),
        calendarHref: calendarHref(event, eventPageShareUrl(origin, event), occurrence, locale),
        jumpIn: occurrence.live ? jumpIn(eventTarget(event)) : undefined,
      };
    }),
  }));

  const setDayOffset = (next: number) =>
    setParams(
      (prev) => {
        const out = new URLSearchParams(prev);
        if (next > 0) out.set(DAY_PARAM, String(next));
        else out.delete(DAY_PARAM);
        return out;
      },
      { preventScrollReset: true },
    );

  const myCards = useMemo(
    () =>
      identity && mine.events ? myEventCards(mine.events, { signer: identity.signer, now, zone, pageOrigin, profiles, locale }) : [],
    [identity, mine.events, now, zone, pageOrigin, profiles, locale],
  );

  const calendarStatus = calendar.data ? "ready" : calendar.isError || initial?.unavailable ? "unavailable" : "loading";

  return (
    <EventsPage
      title={text("Events")}
      notice={notice}
      dialog={
        <>
          {eventId && detail.data ? (
            <EventDetail
              sid={sid}
              event={detail.data}
              zone={zone}
              origin={origin}
              pageOrigin={pageOrigin}
              jumpIn={jumpIn}
              reminder={detail.data.live ? undefined : reminders.get(detail.data.id)}
              profiles={profiles}
              onClose={close}
            />
          ) : null}
          {placeTargetParam && place.data ? (
            <PlaceDetail
              place={place.data}
              parcel={"position" in placeTargetParam ? placeTargetParam.position : undefined}
              origin={origin}
              jumpIn={jumpIn}
              profiles={profiles}
              onClose={close}
            />
          ) : null}
        </>
      }
    >
      <LiveNow cards={liveCards} />
      <Upcoming cards={upcomingCards} />
      <AllEvents
        id={ALL_EVENTS_ID}
        days={allDays}
        layout={columns === 1 ? "list" : "columns"}
        canGoBack={offset > 0}
        canGoForward={offset < MAX_DAY_OFFSET}
        onPrevious={() => setDayOffset(Math.max(0, offset - columns))}
        onNext={() => setDayOffset(Math.min(MAX_DAY_OFFSET, offset + columns))}
        views={
          identity
            ? {
                active: mine.active ? "my" : "all",
                hrefs: { all: EVENTS_PATH, my: `${EVENTS_PATH}?${TAB_PARAM}=my` },
                onFollow: (_view, event) => {
                  if (!isPlainClick(event)) return;
                  event.preventDefault();
                  // The tabs swap the section in place, as source: the page keeps its scroll.
                  void navigate(event.currentTarget.getAttribute("href") ?? EVENTS_PATH, {
                    preventScrollReset: true,
                  });
                },
              }
            : undefined
        }
        mine={{
          cards: myCards,
          status: mine.status,
          createHref: CREATE_HREF,
          onRetry: mine.retry,
        }}
        status={calendarStatus}
        onRetry={() => void calendar.refetch()}
      />
      <HostBanner createHref={CREATE_HREF} learnMoreHref={siteUrl(LEARN_MORE_PATH)} />
    </EventsPage>
  );
}

function PlaceDetail({
  place,
  parcel,
  origin,
  jumpIn,
  profiles,
  onClose,
}: {
  profiles: CreatorProfiles;
  place: JumpPlaceRecord;
  /** The parcel the link asked for: a scene spans several, and its base is only one of them. */
  parcel?: [number, number];
  origin: string;
  jumpIn: (target: Target) => JumpIn;
  onClose: () => void;
}) {
  const { locale, text } = useEventsCopy();
  const base = placeTarget({ base_position: place.base_position, world: place.world, world_name: place.world_name });
  const target = parcel && !base.realm ? { coordinates: parcel } : base;
  return (
    <PlaceDetailModal
      title={place.title || target.realm || text("Untitled place")}
      image={place.image}
      creator={placeCreator(place, profiles, locale)}
      location={{ label: locationLabel(target), world: Boolean(target.realm) }}
      description={place.description ?? undefined}
      users={place.user_count ?? undefined}
      jumpIn={jumpIn(target)}
      shareUrl={placeShareUrl(origin, target)}
      onClose={onClose}
    />
  );
}

/**
 * `?tab=my`, as source: a signed-out visitor is sent to sign in and back; a member who signs out while
 * on it falls back to All Events. Arriving at it scrolls to the section once the member's events load.
 */
function useMyEventsTab(identity: AuthIdentity | null) {
  const [params, setParams] = useSearchParams();
  const wanted = params.get(TAB_PARAM) === "my";
  const [hadIdentity, setHadIdentity] = useState(false);
  useSignInRequired(wanted && !hadIdentity, Boolean(identity));
  useEffect(() => {
    if (identity) setHadIdentity(true);
    // Once the tab is dropped after a sign-out, a later link to it asks to sign in again.
    else if (!wanted) setHadIdentity(false);
    else if (hadIdentity)
      setParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.delete(TAB_PARAM);
          return next;
        },
        { replace: true, preventScrollReset: true },
      );
  }, [identity, wanted, hadIdentity, setParams]);

  const active = wanted && Boolean(identity);
  const query = useQuery({ ...myEventsQuery(identity), enabled: active });

  const scrolled = useRef(false);
  const settled = active && !query.isPending;
  useEffect(() => {
    if (!active) {
      scrolled.current = false;
      return;
    }
    if (!settled || scrolled.current) return;
    scrolled.current = true;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
    document.getElementById(ALL_EVENTS_ID)?.scrollIntoView?.({ behavior: reduce ? "auto" : "smooth", block: "start" });
  }, [active, settled]);

  return {
    active,
    events: active ? query.data : undefined,
    status: query.data ? ("ready" as const) : query.isError ? ("unavailable" as const) : ("loading" as const),
    retry: () => void query.refetch(),
  };
}

/** One view event per day window the visitor looks at, as the What's On feed reported per filter. */
function useViewedOnce(sid: string, settled: boolean, liveCount: number, upcomingCount: number, offset: number) {
  const last = useRef<number | null>(null);
  useEffect(() => {
    if (!sid || !settled || last.current === offset) return;
    last.current = offset;
    track(
      "lp_whatson_viewed",
      { live_count: liveCount, upcoming_count: upcomingCount, filter: offset ? `day+${offset}` : null, search: null },
      { sid, story: STORY },
    );
  }, [sid, settled, liveCount, upcomingCount, offset]);
}
