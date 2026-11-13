import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useQuery } from "@tanstack/react-query";

import { AllEvents, type AllEventsDay } from "@dcl/one-ui/apps/sites/events/AllEvents";
import { EventDetailModal, PlaceDetailModal } from "@dcl/one-ui/apps/sites/events/DetailModal";
import { EventsPage } from "@dcl/one-ui/apps/sites/events/EventsPage";
import { HostBanner } from "@dcl/one-ui/apps/sites/events/HostBanner";
import { LiveNow } from "@dcl/one-ui/apps/sites/events/LiveNow";
import { Upcoming } from "@dcl/one-ui/apps/sites/events/Upcoming";
import type {
  ExperienceCardProps,
  JumpIn,
  LiveEventCardProps,
  RemindControl,
  UpcomingEventCardProps,
} from "@dcl/one-ui/apps/sites/events/cards/types";

import { useAuth } from "@/data/lib/auth/context";
import type { JumpPlaceRecord } from "@/data/lib/catalyst/jump/jump";
import type { Event } from "@/data/lib/catalyst/places/events";
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
import type { DetectedPlatform } from "../download/options";
import { jumpInFor, mobileLaunch } from "../jump/mobileLaunch";
import { useCreatorProfiles } from "./useCreatorProfiles";
import { useEventReminders, type Reminder } from "./useEventReminders";
import type { CreatorProfiles } from "@/data/lib/catalyst/social/creator-profiles";
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
  eventPageShareUrl,
  eventTarget,
  eventUsers,
  formatTime,
  heroWhen,
  liveNowItems,
  locationLabel,
  placeCreator,
  placeHref,
  placeShareUrl,
  placeTarget,
  playHref,
  readDayOffset,
  relativeStart,
  scheduleLine,
  TAB_PARAM,
  usersLabel,
  visibleDays,
  type Target,
  type Zone,
} from "./view";

const STORY: StoryId = "admin/whats-on";
const DETAIL_STORY: StoryId = "admin/whats-on-detail";
const REMIND_FAILED = "Couldn't update your reminder. Try again.";
const LEARN_MORE_URL = "https://decentraland.org/blog/about-decentraland/how-to-make-and-run-an-event-in-decentraland";
const CREATE_HREF = "/events/new-event";

export type EventsViewProps = {
  sid: string;
  platform: DetectedPlatform;
  /** The public origin share links and calendar entries point at. */
  origin: string;
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

/** A plain click opens the detail in place; a modified click keeps the browser's own behaviour. */
function isPlainClick(event: MouseEvent<HTMLAnchorElement>): boolean {
  return !(
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  );
}

function remindControl(reminder: Reminder): RemindControl {
  return { active: reminder.active, pending: reminder.pending, onToggle: reminder.toggle };
}

export function EventsView({ sid, platform, origin, initial }: EventsViewProps) {
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

  const jumpIn = useCallback(
    (target: Target): JumpIn => {
      const { jumpInHref, onJumpIn } = jumpInFor(playHref(target), mobileLaunch(platform, target));
      return { href: jumpInHref ?? playHref(target), onJumpIn };
    },
    [platform],
  );

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
  const reminders = useEventReminders(identity, () => openSignIn({ inPlace: true }));

  const [notice, setNotice] = useState<string>();
  useEffect(() => {
    setNotice(reminders.failed ? REMIND_FAILED : undefined);
  }, [reminders.failed]);

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
    ],
    [liveItems, upcomingEvents, buckets, detail.data, place.data],
  );
  const profiles = useCreatorProfiles(creatorGroups);
  // Counted once both reads settled, so a client navigation never reports the empty first render.
  useViewedOnce(sid, !live.isPending && !upcoming.isPending, liveItems.length, upcomingEvents.length, offset);
  const liveCards: LiveEventCardProps[] = liveItems.map((item, index): LiveEventCardProps => {
    if (item.kind === "event") {
      const href = eventHref(item.event.id, search);
      return {
        id: item.event.id,
        title: item.event.name ?? "Untitled event",
        image: item.event.image,
        href,
        onOpen: open(href, item.event.id),
        users: item.users,
        usersLabel: item.users === undefined ? undefined : usersLabel(item.users),
        creator: eventCreator(item.event, profiles),
        live: true,
        jumpIn: jumpIn(eventTarget(item.event)),
        eager: index === 0,
      };
    }
    const target = placeTarget(item.place);
    const href = placeHref(target, search);
    return {
      id: item.place.id,
      title: item.place.title ?? "Untitled place",
      image: item.place.image,
      href,
      onOpen: open(href),
      users: item.users,
      usersLabel: usersLabel(item.users),
      creator: placeCreator(item.place, profiles),
      live: false,
      jumpIn: jumpIn(target),
      eager: index === 0,
    };
  });

  const upcomingCards: UpcomingEventCardProps[] = upcomingEvents.map((event) => {
    const href = eventHref(event.id, search);
    const start = event.next_start_at ?? event.start_at;
    return {
      id: event.id,
      title: event.name ?? "Untitled event",
      image: event.image,
      href,
      onOpen: open(href, event.id),
      creator: eventCreator(event, profiles),
      time: start ? { label: relativeStart(start, now, zone), dateTime: start } : { label: "Soon" },
      remind: remindControl(reminders.get(event.id)),
      calendarHref: calendarHref(event, eventPageShareUrl(origin, event)),
    };
  });

  const allDays: AllEventsDay[] = days.map((day, i) => ({
    key: day.key,
    label: dayLabel(day.date, offset + i),
    fullLabel: dayFullLabel(day.date),
    isToday: offset + i === 0,
    cards: (buckets[i] ?? []).map((occurrence): ExperienceCardProps => {
      const { event } = occurrence;
      const href = eventHref(event.id, search);
      const users = occurrence.live ? eventUsers(event) : undefined;
      return {
        id: `${event.id}@${occurrence.start}`,
        title: event.name ?? "Untitled event",
        image: event.image,
        href,
        onOpen: open(href, event.id),
        users,
        usersLabel: users === undefined ? undefined : usersLabel(users),
        creator: eventCreator(event, profiles),
        live: occurrence.live,
        time: {
          label: columns === 1 ? relativeStart(occurrence.start, now, zone) : formatTime(occurrence.start, zone),
          dateTime: occurrence.start,
        },
        remind: occurrence.live ? undefined : remindControl(reminders.get(event.id)),
        calendarHref: calendarHref(event, eventPageShareUrl(origin, event), occurrence),
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

  const calendarStatus = calendar.data ? "ready" : calendar.isError || initial?.unavailable ? "unavailable" : "loading";

  return (
    <EventsPage
      title="Events"
      notice={notice}
      dialog={
        <>
          {eventId && detail.data ? (
            <EventDetail
              sid={sid}
              event={detail.data}
              zone={zone}
              origin={origin}
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
        days={allDays}
        layout={columns === 1 ? "list" : "columns"}
        canGoBack={offset > 0}
        canGoForward={offset < MAX_DAY_OFFSET}
        onPrevious={() => setDayOffset(Math.max(0, offset - columns))}
        onNext={() => setDayOffset(Math.min(MAX_DAY_OFFSET, offset + columns))}
        views={
          identity
            ? {
                active: "all",
                hrefs: { all: EVENTS_PATH, my: `${EVENTS_PATH}?${TAB_PARAM}=my` },
                onFollow: (_view, event) => {
                  if (!isPlainClick(event)) return;
                  event.preventDefault();
                  void navigate(event.currentTarget.getAttribute("href") ?? EVENTS_PATH);
                },
              }
            : undefined
        }
        status={calendarStatus}
        onRetry={() => void calendar.refetch()}
      />
      <HostBanner createHref={CREATE_HREF} learnMoreHref={LEARN_MORE_URL} />
    </EventsPage>
  );
}

function EventDetail({
  sid,
  event,
  zone,
  origin,
  jumpIn,
  reminder,
  profiles,
  onClose,
}: {
  profiles: CreatorProfiles;
  sid: string;
  event: Event;
  zone: Zone;
  origin: string;
  jumpIn: (target: Target) => JumpIn;
  reminder?: Reminder;
  onClose: () => void;
}) {
  const target = eventTarget(event);
  const share = eventPageShareUrl(origin, event);
  const viewed = useRef<string | null>(null);
  useEffect(() => {
    if (!sid || viewed.current === event.id) return;
    viewed.current = event.id;
    track("lp_event_viewed", { event_id: event.id, live: event.live }, { sid, story: DETAIL_STORY });
  }, [sid, event.id, event.live]);
  const travel = jumpIn(target);
  const tracked: JumpIn = {
    href: travel.href,
    onJumpIn: (click) => {
      if (sid) {
        track(
          "lp_event_jump_in",
          { event_id: event.id, position: target.realm ? null : `(${target.coordinates.join(",")})` },
          { sid, story: DETAIL_STORY },
        );
      }
      travel.onJumpIn?.(click);
    },
  };
  return (
    <EventDetailModal
      title={event.name ?? "Untitled event"}
      image={event.image}
      live={event.live}
      when={event.live ? undefined : heroWhen(event, zone)}
      creator={eventCreator(event, profiles)}
      location={{
        label: locationLabel(target, event.scene_name ?? event.estate_name),
        world: Boolean(target.realm),
        jumpIn: tracked,
      }}
      description={event.description ?? undefined}
      schedule={scheduleLine(event, zone)}
      jumpIn={tracked}
      remind={reminder ? remindControl(reminder) : undefined}
      calendarHref={calendarHref(event, share)}
      shareUrl={share}
      onClose={onClose}
    />
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
  const base = placeTarget({ base_position: place.base_position, world: place.world, world_name: place.world_name });
  const target = parcel && !base.realm ? { coordinates: parcel } : base;
  return (
    <PlaceDetailModal
      title={place.title || target.realm || "Untitled place"}
      image={place.image}
      creator={placeCreator(place, profiles)}
      location={{ label: locationLabel(target), world: Boolean(target.realm) }}
      description={place.description ?? undefined}
      users={place.user_count ?? undefined}
      jumpIn={jumpIn(target)}
      shareUrl={placeShareUrl(origin, target)}
      onClose={onClose}
    />
  );
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
