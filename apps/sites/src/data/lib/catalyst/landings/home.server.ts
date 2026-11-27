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
import { withDeadline } from "../../request-deadline";
import { VIBE_CREATORS } from "./vibe-creators";
import { parseHomeContent, type HomeContent } from "./home";


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
const HOME_EVENTS_FAILURE_TTL_MS = 5_000;

type HomeData = { content: HomeContent; live: boolean; unreadable: string[] };

// The reference home page's "Jump Into What's Happening": the live
// events, the realm's hot scenes, their places and the creators' profiles, read as /whats-on's Live
// Now reads them.

// The home section has three card places: the list is cut to three.
const LIVE_CARDS = 3;

/** The Catch the Vibe creators, after the cards' creators in the one profile request. */
const VIBE_ADDRESSES = Object.values(VIBE_CREATORS);

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

// The home document waits for this read. 2 s is the deadline the home events read had before it
// became the Live Now join (events, hot scenes, deployers, profiles); it covers the whole chain. A
// node that is slower renders the page with what was read by then, and the short failure hold of
// `homeLiveMemo` asks again.
const HOME_LIVE_TIMEOUT_MS = 2_000;

async function loadHomeLiveNow(opts: GetOptions): Promise<HomeLiveData> {
  const read: Pick<HomeLiveData, "cards" | "profiles" | "unreadable"> = {
    cards: [],
    profiles: {},
    unreadable: ["events"],
  };
  await withDeadline(
    async (signal) => {
      const bounded = { ...opts, signal };
      const { events, scenes } = await readLiveNow(bounded, { world: false });
      signal.throwIfAborted();
      const built = buildHomeLiveCards(events ?? [], scenes ?? []);
      read.cards = built;
      // The creators are still to read: a deadline from here on leaves the cards and names them unread.
      read.unreadable =
        events === null && scenes === null ? ["events"] : ["profiles"];
      const cards =
        (await withDeployers(built, bounded).catch(() => null)) ?? built;
      signal.throwIfAborted();
      read.cards = cards;
      const people = cards.flatMap((card) => [
        ...(card.creatorAddress ? [card.creatorAddress] : []),
        ...(card.attendeeAddresses ?? []),
      ]);
      const profiles =
        (await loadProfiles([...people, ...VIBE_ADDRESSES], bounded).catch(
          () => null,
        )) ?? {};
      signal.throwIfAborted();
      read.profiles = profiles;
      read.unreadable = events === null && scenes === null ? ["events"] : [];
    },
    HOME_LIVE_TIMEOUT_MS,
    opts.signal,
  ).catch(() => {});
  const { cards, profiles, unreadable } = read;
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
  // An expired value serves for one more period while one reload runs behind it, so a visitor waits
  // for the node only when the memo is cold.
  staleMs: HOME_TTL_MS,
  load: (opts: GetOptions) => loadHomeLiveNow(opts),
});

/** Visitor-independent: one read serves a minute of requests, five seconds after an unreadable one. */
export function loadHomeLive(opts: GetOptions = {}): Promise<HomeLiveData> {
  return homeLiveMemo(opts);
}
