import { fetchEvents, type Event } from "../places/events";
import { fetchMostActivePlaces } from "../places/index";
import type { GetOptions } from "../client";
import {
  joinLiveNow,
  loadProfiles,
  plazaCard,
  readLiveNow,
  withDeployers,
  type HotScene,
  type LiveEvent,
  type LiveNowCard,
  type Profile,
} from "../places/live-now.server";
import { ttlMemo } from "../../ttl-memo";
import {
  eventToRitualItem,
  eventWeekday,
  parseHomeContent,
  placeToHotspotItem,
  withHotspots,
  withLiveEvents,
  withRituals,
  type HomeContent,
  type HotspotRailItem,
  type RitualRailItem,
} from "./home";

const LIVE_RAIL_LIMIT = 3;
const HOTSPOT_RAIL_LIMIT = 4;
const RITUAL_RAIL_LIMIT = 5;
const RITUAL_FETCH_LIMIT = 100;
const EVENT_LISTS = ["highlight", "trending", "active"] as const;

const HOME_CONTENT: HomeContent = {
  hero: {
    kicker: "Decentraland now on mobile.",
    title: "Hang Out From Anywhere",
    subtitle: "Close the Feed. Come Hang Out.",
    downloads: null,
    downloadsLabel: "downloads",
    cta: {
      desktop: {
        label: "Jump in \u{2014} no download",
        href: "/play/",
      },
      epic: {
        label: "Play in your browser",
        href: "/play/",
      },
    },
    platforms: [
      { id: "ios", label: "iOS", href: "/play/" },
      { id: "android", label: "Android", href: "/play/" },
    ],
  },
  rails: [
    {
      id: "whatson",
      kind: "events",
      title: "Jump Into What's Happening",
      viewAll: { label: "View All", href: "/whats-on" },
      items: [],
    },
    {
      id: "vibe",
      kind: "hotspots",
      title: "Catch the Vibe",
      viewAll: null,
      items: [],
    },
    {
      id: "rituals",
      kind: "rituals",
      title: "Your Weekly Rituals",
      viewAll: { label: "View All", href: "/whats-on" },
      items: [],
    },
  ],
  comeHangOut: {
    title: "Come Hang Out",
    downloads: null,
    downloadsLabel: "downloads",
  },
};

function loadHomeContent(): HomeContent {
  return parseHomeContent(HOME_CONTENT);
}

const HOME_TTL_MS = 60_000;
const HOME_EVENTS_FAILURE_TTL_MS = 5_000;

type HomeData = { content: HomeContent; live: boolean; unreadable: string[] };

const homeMemo = ttlMemo({
  ttlMs: HOME_TTL_MS,
  keyOf: (opts: GetOptions) => (opts.fetchImpl || opts.base ? null : ""),
  keep: (d: HomeData) => d.unreadable.length === 0,
  load: (opts) => loadHomeRails(opts.fetchImpl || opts.base ? opts : {}),
});

// The rails are visitor-independent; a fully readable result serves a minute of requests.
export function loadHome(opts: GetOptions = {}): Promise<HomeData> {
  return homeMemo(opts);
}

// The reference home page's "Jump Into What's Happening" (sites@17b3535a: features/events/
// events.discovery.ts, events.discovery.helpers.ts, features/profile/profile.client.ts): the live
// events, the realm's hot scenes, their places and the creators' profiles, read as /whats-on's Live
// Now reads them.

const LIVE_CARDS = 3;

/** The Catch the Vibe creators, after the cards' creators in the one profile request. */
export const VIBE_ADDRESSES = [
  "0xd82d005e8f8d5385db40ba23884a5c967bb1e8af",
  "0x54e93609eb454a1f152edefdf022480794ce2130",
];

export type HomeLiveData = HomeData & {
  cards: LiveNowCard[];
  profiles: Record<string, Profile>;
};

/** Hot scenes with at least five people, those with a live event first, then Genesis Plaza when fewer than three. */
export function buildHomeLiveCards(
  liveEvents: LiveEvent[],
  hotScenes: HotScene[],
): LiveNowCard[] {
  const cards = joinLiveNow(liveEvents, hotScenes);
  if (cards.length < LIVE_CARDS) {
    const plaza = plazaCard(hotScenes);
    if (!cards.some((card) => card.id === plaza.id)) cards.push(plaza);
  }
  return cards.slice(0, LIVE_CARDS);
}

/** The cards as the agent markdown's event rail (heroEventCards). */
function withLiveCards(content: HomeContent, cards: LiveNowCard[]): HomeContent {
  const items = cards.map((card) => ({
    id: card.id,
    category: "LIVE",
    title: card.title,
    when: "Live now",
    live: true,
    image: card.image || null,
  }));
  return {
    ...content,
    rails: content.rails.map((rail) =>
      rail.kind === "events" ? { ...rail, items } : rail,
    ),
  };
}

async function loadHomeLiveNow(opts: GetOptions): Promise<HomeLiveData> {
  const { events, scenes } = await readLiveNow(opts, { world: false });
  const unreadable = events === null && scenes === null ? ["events"] : [];
  const built = buildHomeLiveCards(events ?? [], scenes ?? []);
  const cards = (await withDeployers(built, opts).catch(() => null)) ?? built;
  const creators = cards.flatMap((card) =>
    card.creatorAddress ? [card.creatorAddress] : [],
  );
  const profiles =
    (await loadProfiles([...creators, ...VIBE_ADDRESSES], opts).catch(
      () => null,
    )) ?? {};
  return {
    content: withLiveCards(loadHomeContent(), cards),
    live: cards.length > 0,
    unreadable,
    cards,
    profiles,
  };
}

const homeLiveMemo = ttlMemo({
  ttlMs: HOME_TTL_MS,
  ttlMsForValue: (data: HomeLiveData) =>
    data.unreadable.length === 0 ? HOME_TTL_MS : HOME_EVENTS_FAILURE_TTL_MS,
  keyOf: (opts: GetOptions) => (opts.fetchImpl || opts.base ? null : ""),
  load: (opts: GetOptions) => loadHomeLiveNow(opts),
});

/** Visitor-independent: one read serves a minute of requests, five seconds after an unreadable one. */
export function loadHomeLive(opts: GetOptions = {}): Promise<HomeLiveData> {
  return homeLiveMemo(opts);
}

async function loadHomeRails(opts: GetOptions): Promise<HomeData> {
  let content = loadHomeContent();

  const [events, hotspots, rituals] = await Promise.all([
    loadRailEvents(opts),
    loadHotspots(opts),
    loadRituals(opts),
  ]);

  const unreadable: string[] = [];
  const live = (events?.length ?? 0) > 0;
  if (events === null) unreadable.push("events");
  else if (live) content = withLiveEvents(content, events);

  if (hotspots === null) unreadable.push("hotspots");
  else content = withHotspots(content, hotspots);

  if (rituals === null) unreadable.push("rituals");
  else content = withRituals(content, rituals);

  if (unreadable.length > 0) {
    content = {
      ...content,
      rails: content.rails.filter((rail) => !unreadable.includes(rail.kind)),
    };
  }

  return { content, live, unreadable };
}

async function loadRailEvents(opts: GetOptions): Promise<Event[] | null> {
  const results = await Promise.all(
    EVENT_LISTS.map((list) =>
      fetchEvents({ list, limit: LIVE_RAIL_LIMIT }, opts)
        .then((r) => r.data)
        .catch(() => null),
    ),
  );
  if (results.every((events) => events === null)) return null;
  return results.find((events) => events !== null && events.length > 0) ?? [];
}

async function loadHotspots(
  opts: GetOptions,
): Promise<HotspotRailItem[] | null> {
  const places = await fetchMostActivePlaces(
    { limit: HOTSPOT_RAIL_LIMIT },
    opts,
  ).catch(() => null);
  return places === null ? null : places.map(placeToHotspotItem);
}

async function loadRituals(opts: GetOptions): Promise<RitualRailItem[] | null> {
  const events = await fetchEvents({ limit: RITUAL_FETCH_LIMIT }, opts)
    .then((r) => r.data)
    .catch(() => null);
  if (events === null) return null;
  if (!events.length) return [];

  const picked = events.filter((e) => e.recurrent_frequency === "WEEKLY");
  if (picked.length < 2) {
    for (const e of events) {
      if (picked.length >= RITUAL_RAIL_LIMIT) break;
      if (e.recurrent && !picked.some((p) => p.id === e.id)) picked.push(e);
    }
  }
  if (picked.length < 2) {
    for (const e of events) {
      if (picked.length >= RITUAL_RAIL_LIMIT) break;
      if (!picked.some((p) => p.id === e.id)) picked.push(e);
    }
  }

  return sortByWeekday(picked)
    .slice(0, RITUAL_RAIL_LIMIT)
    .map(eventToRitualItem);
}

function weekdayKey(e: Event): number {
  const day = eventWeekday(e);
  return day == null ? 7 : (day + 6) % 7;
}

function sortByWeekday(events: Event[]): Event[] {
  return [...events].sort((a, b) => weekdayKey(a) - weekdayKey(b));
}
