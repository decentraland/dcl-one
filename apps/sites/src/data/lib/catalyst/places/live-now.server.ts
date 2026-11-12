import { z } from "zod";

import { profileFaceUrl } from "../../auth/profile-face-url";
import { catalystBase, getJSON, type GetOptions } from "../client";
import { eventsApiPath, placesApiPath } from "../typed";
import { readCopy, type CopyStamp } from "@ui/data/copyAsOf";

// The Live Now cards, read the way the reference reads them (sites@17b3535a: features/events/events.helpers.ts
// buildLiveNowCards and enrichPlaceCards, features/events/events.client.ts getLiveNowCards, features/profile/
// profile.client.ts), on the server: hot scenes with people, each joined to the live event on one of its parcels.

export const MIN_USERS = 5;
export const DCL_FOUNDATION_NAME = "Decentraland Foundation";

const LiveEvent = z.object({
  id: z.string(),
  name: z.string(),
  image: z.string().nullish(),
  x: z.number(),
  y: z.number(),
  user: z.string().nullish(),
  user_name: z.string().nullish(),
});
export type LiveEvent = z.infer<typeof LiveEvent>;
const LiveEvents = z.object({ data: z.array(z.unknown()) });

const HotScene = z.object({
  id: z.string(),
  name: z.string(),
  baseCoords: z.tuple([z.number(), z.number()]),
  usersTotalCount: z.number(),
  parcels: z.array(z.tuple([z.number(), z.number()])),
  thumbnail: z.string().nullish(),
});
export type HotScene = z.infer<typeof HotScene>;

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
const Places = z.object({
  data: z.array(z.object({ owner: z.string().nullish(), contact_name: z.string().nullish(), creator_address: z.string().nullish() })),
});

/** A Live Now card: a live event in a scene with people, or such a scene on its own. */
export type LiveNowCard = {
  id: string;
  title: string;
  users: number;
  /** Own-origin image URL, or "". */
  image: string;
  /** "x,y" */
  coordinates: string;
  creatorAddress?: string;
  creatorName?: string;
  /** The live event's `user_name`, which the reference's events page shows before any profile name. */
  userName?: string;
  isGenesisPlaza: boolean;
  /** A scene card (true) or a live event card (false). */
  place: boolean;
};

/** `address` as the profile states it: the reference hashes it, not the asked one, into the avatar colour. */
export type Profile = { address: string; name: string | null; hasClaimedName: boolean; face: string | null };

const ETH_ADDRESS = /^0x[0-9a-fA-F]{40}$/;
export const isEthAddress = (value: string | null | undefined): value is string => !!value && ETH_ADDRESS.test(value);
export const formatEthAddress = (address: string) => `${address.slice(0, 6)}…${address.slice(-4)}`;

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
    image: ownContent(plaza?.thumbnail),
    coordinates: plaza ? coordsKey(...plaza.baseCoords) : "0,0",
    creatorName: DCL_FOUNDATION_NAME,
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
      creatorName: genesis ? DCL_FOUNDATION_NAME : undefined,
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

export async function postJSON(path: string, body: unknown, opts: GetOptions): Promise<unknown> {
  const res = await (opts.fetchImpl ?? fetch)(`${catalystBase(opts.base)}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json" },
    body: JSON.stringify(body),
    signal: opts.signal,
  });
  if (!res.ok) throw new Error(`${path} returned ${res.status}`);
  return res.json();
}

/**
 * The live events and the hot scenes; a failed read is null, so a page shows what the other read found, as the
 * reference does. `world: false` keeps Genesis City events only (the home page); the events page reads both.
 */
export async function readLiveNow(opts: GetOptions, query: { world?: false } = {}): Promise<{ events: LiveEvent[] | null; scenes: HotScene[] | null; copy: CopyStamp | null }> {
  let copy: CopyStamp | null = null;
  const [events, scenes] = await Promise.all([
    getJSON<unknown>(eventsApiPath("get", "/v1/events"), { ...opts, query: { list: "live", limit: 20, order: "asc", ...query } })
      .then((raw) => {
        copy = readCopy(raw);
        return parsed(LiveEvent, LiveEvents.parse(raw).data);
      })
      .catch(() => null),
    getJSON<unknown>("/hot-scenes", opts)
      .then((raw) => parsed(HotScene, z.array(z.unknown()).parse(raw)))
      .catch(() => null),
  ]);
  return { events, scenes, copy };
}

/**
 * Each place card's creator from the places catalogue (`creator_address`, else a wallet `owner`) and its name
 * (`contact_name`, else an owner that is not a wallet), as the reference's enrichPlaceCards reads them.
 */
export async function withPlaces(cards: LiveNowCard[], opts: GetOptions): Promise<LiveNowCard[]> {
  return Promise.all(cards.map(async (card) => {
    if (!card.place || (card.creatorAddress && card.creatorName)) return card;
    const place = await getJSON<unknown>(placesApiPath("get", "/api/places"), { ...opts, query: { positions: card.coordinates } })
      .then((raw) => Places.parse(raw).data[0])
      .catch(() => undefined);
    if (!place) return card;
    const owner = place.owner?.trim() || undefined;
    const creator = place.creator_address?.trim();
    const address = isEthAddress(creator) ? creator : isEthAddress(owner) ? owner : undefined;
    const name = place.contact_name?.trim() || (owner && !isEthAddress(owner) ? owner : undefined);
    return { ...card, creatorAddress: card.creatorAddress ?? address, creatorName: card.creatorName ?? name };
  }));
}

/** The deployer of each place card's scene, when nothing else named a creator: its active entity, then that entity's deployment. */
export async function withDeployers(cards: LiveNowCard[], opts: GetOptions): Promise<LiveNowCard[]> {
  const pointers = cards.filter((c) => c.place && !c.creatorAddress).map((c) => c.coordinates);
  if (pointers.length === 0) return cards;
  const entities = parsed(ActiveEntity, z.array(z.unknown()).parse(await postJSON("/content/entities/active", { pointers }, opts)));
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
  const rows = Profiles.parse(await postJSON("/lambdas/profiles", { ids }, opts));
  const out: Record<string, Profile> = {};
  for (const row of rows) {
    const a = row.avatars[0];
    if (!a?.ethAddress) continue;
    out[a.ethAddress.toLowerCase()] = { address: a.ethAddress, name: a.name ?? null, hasClaimedName: a.hasClaimedName ?? false, face: profileFaceUrl(a.avatar?.snapshots?.face256 ?? undefined, "") || null };
  }
  return out;
}

/** The events page's Live Now: both Genesis City and world events, place cards credited from the catalogue, then the deployer. */
export async function loadLiveNowCards(opts: GetOptions = {}): Promise<{ cards: LiveNowCard[]; copy: CopyStamp | null }> {
  const { events, scenes, copy } = await readLiveNow(opts);
  if (!events && !scenes) throw new Error("live now unavailable");
  const cards = buildLiveNowCards(events ?? [], scenes ?? []);
  const placed = await withPlaces(cards, opts);
  return { cards: await withDeployers(placed, opts).catch(() => placed), copy };
}
