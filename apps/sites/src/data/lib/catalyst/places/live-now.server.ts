import { z } from "zod";
import { genesisPlazaUrl as plazaThumbnail } from "@dcl/one-ui";

import { profileFaceUrl } from "../../auth/profile-face-url";
import { withDeadline } from "../../request-deadline";
import { ttlMemo } from "../../ttl-memo";
import { FOUNDATION_CONTACT_NAME } from "../jump/jump";
import { getJSON, postPublicJSON, type GetOptions } from "../client";
import { eventsApiPath, placesApiPath } from "../typed";
import { HOT_SCENES_PATH, HotScene, hotSceneRows } from "./hot-scenes";
import { readCopy, type CopyStamp } from "@web-core/freshness/copyAsOf";

// The Live Now cards, read the way the reference reads them, on the server: hot scenes with people, each joined to the live event on one of its parcels.

const LiveEvent = z.object({
  id: z.string(),
  name: z.string(),
  image: z.string().nullish(),
  x: z.number(),
  y: z.number(),
  user: z.string().nullish(),
  user_name: z.string().nullish(),
  start_at: z.string().nullish().catch(undefined),
  next_start_at: z.string().nullish().catch(undefined),
  recurrent: z.boolean().nullish().catch(undefined),
  latest_attendees: z.array(z.unknown()).nullish().catch(undefined),
});
export type LiveEvent = z.infer<typeof LiveEvent>;
const LiveEvents = z.object({ data: z.array(z.unknown()) });

export type { HotScene };

/** A hot scene with fewer people than this is not live. */
const MIN_USERS = 5;
/** A card shows a sample of the public RSVP list. */
const MAX_ATTENDEE_FACES = 3;

const Places = z.object({
  data: z.array(
    z.object({
      owner: z.string().nullish(),
      contact_name: z.string().nullish(),
      creator_address: z.string().nullish(),
      positions: z.array(z.string()).nullish(),
      base_position: z.string().nullish(),
    }),
  ),
});
/** The largest page of the places list; the hot scenes list holds no more scenes than this. */
const PLACES_LIMIT = 100;

const ETH_ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const isEthAddress = (value: unknown): value is string => typeof value === "string" && ETH_ADDRESS.test(value);
export const formatEthAddress = (address: string) => `${address.slice(0, 6)}…${address.slice(-4)}`;

const ActiveEntity = z.object({ id: z.string(), pointers: z.array(z.string()) });
const Deployments = z.object({
  deployments: z.array(z.object({ entityId: z.string().nullish(), deployedBy: z.string().nullish() })),
});
const Profiles = z.array(
  z.object({
    avatars: z.array(
      z.object({
        name: z.string().nullish(),
        hasClaimedName: z.boolean().nullish(),
        ethAddress: z.string().nullish(),
        avatar: z.object({ snapshots: z.object({ face256: z.string().nullish() }).nullish() }).nullish(),
      }),
    ),
  }),
);
/** A Live Now card: a live event in a scene with people, or such a scene on its own. */
export type LiveNowCard = {
  id: string;
  title: string;
  users: number;
  /** The scene's thumbnail (a content file at the node's own origin) or the event's poster as the events API names it, or "". */
  image: string;
  /** "x,y" */
  coordinates: string;
  creatorAddress?: string;
  creatorName?: string;
  /** The live event's `user_name`, which the reference's events page shows before any profile name. */
  userName?: string;
  startedAt?: string;
  attendeeAddresses?: string[];
  isGenesisPlaza: boolean;
  /** A scene card (true) or a live event card (false). */
  place: boolean;
};

/** `address` as the profile states it: the reference hashes it, not the asked one, into the avatar colour. */
export type Profile = { address: string; name: string | null; hasClaimedName: boolean; face: string | null };

/** A content-server file URL as the node serves it at its own origin. */
export const ownContent = (url: string | null | undefined) => {
  const hash = /\/contents\/+([A-Za-z0-9]+)$/.exec(url ?? "")?.[1];
  return hash ? `/content/contents/${hash}` : (url ?? "");
};

export const coordsKey = (x: number, y: number) => `${x},${y}`;
const isGenesisPlaza = (scene: HotScene) => scene.name.toLowerCase().includes("genesis plaza");

/** Genesis Plaza, credited to the Foundation, from the hot scenes when they list it. */
export function plazaCard(hotScenes: HotScene[]): LiveNowCard {
  const plaza = hotScenes.find(isGenesisPlaza);
  return {
    id: plaza?.id ?? "genesis-plaza",
    title: plaza?.name ?? "Genesis Plaza",
    users: plaza?.usersTotalCount ?? 0,
    image: ownContent(plaza?.thumbnail) || plazaThumbnail,
    coordinates: plaza ? coordsKey(...plaza.baseCoords) : "0,0",
    creatorName: FOUNDATION_CONTACT_NAME,
    isGenesisPlaza: true,
    place: true,
  };
}

/**
 * Hot scenes with at least five people: those holding a live event as event cards (the scene's count, most
 * people first), the rest as place cards, most people first.
 */
export function joinLiveNow(liveEvents: LiveEvent[], hotScenes: HotScene[]): LiveNowCard[] {
  const scenes = hotScenes.filter((s) => s.usersTotalCount >= MIN_USERS);
  const cards: LiveNowCard[] = [];
  const usedScenes = new Set<string>();
  const usedEvents = new Set<string>();

  for (const scene of scenes) {
    const parcel = scene.parcels.find(([px, py]) => liveEvents.some((e) => e.x === px && e.y === py));
    const event = parcel && liveEvents.find((e) => e.x === parcel[0] && e.y === parcel[1]);
    if (event && !usedEvents.has(event.id)) {
      cards.push({
        id: event.id,
        title: event.name,
        users: scene.usersTotalCount,
        image: ownContent(event.image),
        coordinates: coordsKey(event.x, event.y),
        creatorAddress: event.user ?? undefined,
        userName: event.user_name || undefined,
        startedAt: event.next_start_at ?? (event.recurrent === false ? event.start_at ?? undefined : undefined),
        attendeeAddresses: event.latest_attendees
          ? [
              ...new Set(
                event.latest_attendees
                  .filter(isEthAddress)
                  .map((address) => address.toLowerCase()),
              ),
            ].slice(0, MAX_ATTENDEE_FACES)
          : undefined,
        isGenesisPlaza: false,
        place: false,
      });
      usedScenes.add(scene.id);
      usedEvents.add(event.id);
    }
  }
  cards.sort((a, b) => b.users - a.users);

  const rest = scenes.filter((s) => !usedScenes.has(s.id)).sort((a, b) => b.usersTotalCount - a.usersTotalCount);
  for (const scene of rest) {
    const genesis = isGenesisPlaza(scene);
    cards.push({
      id: scene.id,
      title: scene.name,
      users: scene.usersTotalCount,
      image: ownContent(scene.thumbnail),
      coordinates: coordsKey(...scene.baseCoords),
      creatorName: genesis ? FOUNDATION_CONTACT_NAME : undefined,
      isGenesisPlaza: genesis,
      place: true,
    });
  }
  return cards;
}

/** The reference's events page: the join, or Genesis Plaza when it yields nothing. */
export function buildLiveNowCards(liveEvents: LiveEvent[], hotScenes: HotScene[]): LiveNowCard[] {
  const cards = joinLiveNow(liveEvents, hotScenes);
  return cards.length ? cards : [plazaCard(hotScenes)];
}

export function parsed<T>(schema: z.ZodType<T>, rows: unknown[]): T[] {
  return rows.flatMap((row) => {
    const r = schema.safeParse(row);
    return r.success ? [r.data] : [];
  });
}

/**
 * The live events and the hot scenes; a failed read is null, so a page shows what the other read found, as the
 * reference does. `world: false` keeps Genesis City events only (the home page); the events page reads both.
 */
export async function readLiveNow(opts: GetOptions, query: { world?: false } = {}): Promise<{ events: LiveEvent[] | null; scenes: HotScene[] | null; copy: CopyStamp | null }> {
  let copy: CopyStamp | null = null;
  // 20 is one page of the live list, the page decentraland.org reads: the join needs each live event that can sit
  // in a busy scene, which is more than the cards a rail shows.
  const [events, scenes] = await Promise.all([
    getJSON<unknown>(eventsApiPath("get", "/v1/events"), { ...opts, query: { list: "live", limit: 20, order: "asc", ...query } })
      .then((raw) => {
        copy = readCopy(raw);
        return parsed(LiveEvent, LiveEvents.parse(raw).data);
      })
      .catch(() => null),
    getJSON<unknown>(HOT_SCENES_PATH, opts)
      .then((raw) => hotSceneRows(HotScene, raw))
      .catch(() => null),
  ]);
  return { events, scenes, copy };
}

/**
 * Each place card's creator from the places catalogue (`creator_address`, else a wallet `owner`) and its name
 * (`contact_name`, else an owner that is not a wallet), as the reference's enrichPlaceCards reads them. One
 * request asks for the places of all the cards; a card's place is the first one that holds the card's parcel.
 */
export async function withPlaces(cards: LiveNowCard[], opts: GetOptions): Promise<LiveNowCard[]> {
  const asks = (card: LiveNowCard) => card.place && !(card.creatorAddress && card.creatorName);
  const positions = [...new Set(cards.filter(asks).map((card) => card.coordinates))];
  if (positions.length === 0) return cards;
  const query = new URLSearchParams([...positions.map((position) => ["positions", position]), ["limit", String(PLACES_LIMIT)]]);
  const places = Places.parse(await getJSON<unknown>(`${placesApiPath("get", "/api/places")}?${query}`, opts)).data;
  const placeAt = new Map<string, (typeof places)[number]>();
  for (const place of places) {
    for (const position of [...(place.positions ?? []), place.base_position]) if (position && !placeAt.has(position)) placeAt.set(position, place);
  }
  return cards.map((card) => {
    const place = asks(card) ? placeAt.get(card.coordinates) : undefined;
    if (!place) return card;
    const owner = place.owner?.trim() || undefined;
    const creator = place.creator_address?.trim();
    const address = isEthAddress(creator) ? creator : isEthAddress(owner) ? owner : undefined;
    const name = place.contact_name?.trim() || (owner && !isEthAddress(owner) ? owner : undefined);
    return { ...card, creatorAddress: card.creatorAddress ?? address, creatorName: card.creatorName ?? name };
  });
}

/** The deployer of each place card's scene, when nothing else named a creator: its active entity, then that entity's deployment. */
export async function withDeployers(cards: LiveNowCard[], opts: GetOptions): Promise<LiveNowCard[]> {
  const pointers = cards.filter((c) => c.place && !c.creatorAddress).map((c) => c.coordinates);
  if (pointers.length === 0) return cards;
  const entities = parsed(ActiveEntity, z.array(z.unknown()).parse(await postPublicJSON("/content/entities/active", { pointers }, opts)));
  if (entities.length === 0) return cards;
  const query = new URLSearchParams(entities.map((e) => ["entityId", e.id]));
  const { deployments } = Deployments.parse(await getJSON(`/content/deployments/?${query}`, opts));
  const deployer = new Map<string, string>();
  for (const entity of entities) {
    const by = deployments.find((d) => d.entityId === entity.id)?.deployedBy;
    if (by) for (const p of entity.pointers) if (pointers.includes(p)) deployer.set(p, by);
  }
  return cards.map((c) => (c.place && !c.creatorAddress && deployer.has(c.coordinates) ? { ...c, creatorAddress: deployer.get(c.coordinates) } : c));
}

/** The profiles of `addresses`, one `POST /lambdas/profiles`, keyed by lowercase address; faces at the node's own origin. */
export async function loadProfiles(addresses: string[], opts: GetOptions): Promise<Record<string, Profile>> {
  const ids = [...new Set(addresses.map((a) => a.toLowerCase()))];
  if (ids.length === 0) return {};
  const rows = Profiles.parse(await postPublicJSON("/lambdas/profiles", { ids }, opts));
  const out: Record<string, Profile> = {};
  for (const row of rows) {
    const a = row.avatars[0];
    if (!a?.ethAddress) continue;
    out[a.ethAddress.toLowerCase()] = { address: a.ethAddress, name: a.name ?? null, hasClaimedName: a.hasClaimedName ?? false, face: profileFaceUrl(a.avatar?.snapshots?.face256 ?? undefined, "") || null };
  }
  return out;
}

/** The whole read has the deadline of the home page's live read. */
const LIVE_NOW_TIMEOUT_MS = 2_000;
const LIVE_NOW_TTL_MS = 60_000;
const LIVE_NOW_RETRY_TTL_MS = 5_000;

/** `whole` is false when a read failed or passed the deadline. */
type LiveNow = { cards: LiveNowCard[]; copy: CopyStamp | null; whole: boolean };
/** No request signal: a visitor who leaves does not end the read that other visitors share. */
type LiveNowOptions = Omit<GetOptions, "signal">;

/**
 * The events page's Live Now: both Genesis City and world events, place cards credited from the catalogue, then
 * the deployer. It rejects when neither the events nor the hot scenes were read by the deadline; a creators' read
 * that fails or passes the deadline leaves the cards without them.
 */
async function readLiveNowCards(opts: LiveNowOptions): Promise<LiveNow> {
  const read: { live: LiveNow | null } = { live: null };
  await withDeadline(async (signal) => {
    const bounded = { ...opts, signal };
    const { events, scenes, copy } = await readLiveNow(bounded);
    signal.throwIfAborted();
    if (!events && !scenes) return;
    const built = buildLiveNowCards(events ?? [], scenes ?? []);
    read.live = { cards: built, copy, whole: false };
    const placed = await withPlaces(built, bounded).catch(() => null);
    signal.throwIfAborted();
    read.live = { cards: placed ?? built, copy, whole: false };
    const credited = await withDeployers(placed ?? built, bounded).catch(() => null);
    signal.throwIfAborted();
    read.live = { cards: credited ?? placed ?? built, copy, whole: !!(events && scenes && placed && credited) };
  }, LIVE_NOW_TIMEOUT_MS).catch(() => {});
  if (!read.live) throw new Error("live now unavailable");
  return read.live;
}

const liveNowMemo = ttlMemo({
  ttlMs: LIVE_NOW_TTL_MS,
  ttlMsForValue: (live: LiveNow) => (live.whole ? LIVE_NOW_TTL_MS : LIVE_NOW_RETRY_TTL_MS),
  keyOf: (opts: LiveNowOptions) => (opts.fetchImpl || opts.base ? null : ""),
  // An expired value serves for one more period while one reload runs behind it, so a visitor waits for the node
  // only when the memo is cold.
  staleMs: LIVE_NOW_TTL_MS,
  load: readLiveNowCards,
});

/** Visitor-independent: one read serves a minute of requests, five seconds when a part of it was not read. */
export function loadLiveNowCards(opts: LiveNowOptions = {}): Promise<{ cards: LiveNowCard[]; copy: CopyStamp | null }> {
  return liveNowMemo(opts);
}

export function resetLiveNowCards(): void {
  liveNowMemo.reset();
}
