import { fetchEvents, type Event } from "../places/events";
import { fetchMostActivePlaces } from "../places/index";
import type { GetOptions } from "../client";
import { ttlMemo } from "../../ttl-memo";
import { loadDownloadsLabel } from "./download-counts.server";
import {
  buildWhatsOnCards,
  withCreators,
  type WhatsOnCard,
} from "./events.server";
import {
  eventToRitualItem,
  eventWeekday,
  parseHomeContent,
  placeToHotspotItem,
  withHotspots,
  withLiveEvents,
  withRituals,
  type HomeContent,
  type RitualRailItem,
} from "./home";

const LIVE_RAIL_LIMIT = 3;
const HOTSPOT_RAIL_LIMIT = 4;
const RITUAL_RAIL_LIMIT = 5;
const RITUAL_FETCH_LIMIT = 100;
const EVENT_LISTS = ["highlight", "trending", "active"] as const;
// Source Sites reads up to 20 live events to match against crowded scenes.
const WHATS_ON_LIVE_LIMIT = 20;

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
      viewAll: { label: "View All", href: "/events" },
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
      viewAll: { label: "View All", href: "/events" },
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

type HomeData = {
  content: HomeContent;
  live: boolean;
  /** Failed reads, including live-events; successful empty lists are readable. */
  unreadable: string[];
  /** The source Sites home's "What's on" cards. */
  whatsOn: WhatsOnCard[];
  /** The Explorer download counter, or the source's static label. */
  downloadsLabel: string;
};

const homeMemo = ttlMemo({
  ttlMs: HOME_TTL_MS,
  keyOf: (opts: GetOptions) => (opts.fetchImpl || opts.base ? null : ""),
  keep: (d: HomeData) => d.unreadable.length === 0,
  load: (opts) => loadHomeLive(opts.fetchImpl || opts.base ? opts : {}),
});

// The rails are visitor-independent; a fully readable result serves a minute of requests.
export function loadHome(opts: GetOptions = {}): Promise<HomeData> {
  return homeMemo(opts);
}

async function loadHomeLive(opts: GetOptions): Promise<HomeData> {
  let content = loadHomeContent();

  const [events, places, rituals, liveEvents, downloadsLabel] =
    await Promise.all([
      loadRailEvents(opts),
      fetchMostActivePlaces({ limit: HOTSPOT_RAIL_LIMIT }, opts).catch(
        () => null,
      ),
      loadRituals(opts),
      fetchEvents({ list: "live", limit: WHATS_ON_LIVE_LIMIT }, opts)
        .then((r) => r.data)
        .catch(() => null),
      loadDownloadsLabel(),
    ]);
  const hotspots = places === null ? null : places.map(placeToHotspotItem);
  const cards = buildWhatsOnCards(liveEvents ?? [], places ?? []);
  const whatsOn = cards.length ? await withCreators(cards, opts) : cards;

  const unreadable: string[] = [];
  const live = (events?.length ?? 0) > 0;
  if (liveEvents === null) unreadable.push("live-events");
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

  return { content, live, unreadable, whatsOn, downloadsLabel };
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
