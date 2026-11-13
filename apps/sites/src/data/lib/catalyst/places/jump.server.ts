// from: sites@17b3535a, features/places/places.client.ts and places.mappers.ts. The deep-link cards read what
// decentraland.org reads, in its request shapes: the place or event, then the scene deployer's (or the event
// creator's) catalyst profile.
import { z } from "zod";

import { avatarBackground, displayName } from "@ui/web/places/avatarColor";

import { profileFaceUrl } from "../../auth/profile-face-url";
import { catalystBase, getJSON, worldsBase, type GetOptions } from "../client";
import { playUrl } from "../bases";
import { eventsApiPath, placesApiPath } from "../typed";
import { isEns, type JumpCardData, type JumpTarget } from "./jump";

const FOUNDATION_CONTACT_NAME = "Decentraland Foundation";
const ADDRESS = /^0x[a-fA-F0-9]{40}$/;

const Place = z.object({
  title: z.string().nullish(),
  description: z.string().nullish(),
  image: z.string().nullish(),
  owner: z.string().nullish(),
  contact_name: z.string().nullish(),
  base_position: z.string(),
  user_count: z.number().nullish(),
  world: z.boolean().nullish(),
  world_name: z.string().nullish(),
});
type Place = z.infer<typeof Place>;
const Places = z.object({ data: z.array(Place).nullish() });

const Event = z.object({
  id: z.string(),
  name: z.string(),
  user: z.string().nullish(),
  user_name: z.string().nullish(),
  image: z.string().nullish(),
  description: z.string().nullish(),
  next_start_at: z.string(),
  next_finish_at: z.string(),
  coordinates: z.tuple([z.number(), z.number()]),
  total_attendees: z.number().nullish(),
  live: z.boolean().nullish(),
  world: z.boolean().nullish(),
  server: z.string().nullish(),
  scene_name: z.string().nullish(),
  deleted_by_user: z.boolean().nullish(),
  deleted_by_admin: z.boolean().nullish(),
});
type Event = z.infer<typeof Event>;

const Profiles = z.array(
  z.object({
    avatars: z
      .array(
        z.object({
          name: z.string().nullish(),
          realName: z.string().nullish(),
          hasClaimedName: z.boolean().nullish(),
          ethAddress: z.string().nullish(),
          avatar: z.object({ snapshots: z.object({ face256: z.string().nullish() }).nullish() }).nullish(),
        }),
      )
      .nullish(),
  }),
);

type Creator = { user: string; name: string; avatar?: string; color: string };

async function post(url: string, body: unknown, opts: GetOptions): Promise<Response> {
  return (opts.fetchImpl ?? fetch)(url, {
    method: "POST",
    headers: { accept: "*/*", "content-type": "application/json" },
    body: JSON.stringify(body),
    signal: opts.signal,
  });
}

/** The catalyst profile of `address`, or nothing when it has no name (curated scenes, some World owners). */
async function loadCreator(address: string, opts: GetOptions): Promise<Creator | undefined> {
  const res = await post(`${catalystBase(opts.base)}/lambdas/profiles`, { ids: [address] }, opts);
  if (!res.ok) return undefined;
  const avatar = Profiles.safeParse(await res.json()).data?.[0]?.avatars?.[0];
  const name = avatar?.name || avatar?.realName;
  if (!name) return undefined;
  const color = avatarBackground(displayName(avatar?.name, !!avatar?.hasClaimedName, avatar?.ethAddress ?? address));
  return { user: address, name, avatar: profileFaceUrl(avatar?.avatar?.snapshots?.face256 ?? undefined) || undefined, color };
}

/** The scene at `position`: its active entity, that entity's deployment, then the deployer's profile. */
async function loadDeployer(position: string, opts: GetOptions): Promise<Creator | undefined> {
  const entities = await post(`${catalystBase(opts.base)}/content/entities/active`, { pointers: [position] }, opts);
  if (!entities.ok) return undefined;
  const entityId = z.array(z.object({ id: z.string() })).safeParse(await entities.json()).data?.[0]?.id;
  if (!entityId) return undefined;
  const deployments = await getJSON<unknown>("/content/deployments/", { ...opts, query: { entityId } }).catch(() => null);
  const deployedBy = z.object({ deployments: z.array(z.object({ deployedBy: z.string() })) }).safeParse(deployments).data?.deployments[0]?.deployedBy;
  return deployedBy ? loadCreator(deployedBy, opts) : undefined;
}

/** A World's owner, from its scene on the worlds content server; "missing" when the World has no scene. */
async function loadWorldOwner(world: string, opts: GetOptions): Promise<Creator | undefined | "missing"> {
  const res = await post(`${worldsBase()}/entities/active`, { pointers: [world.toLowerCase()] }, opts);
  if (!res.ok) return undefined;
  const entity = z.array(z.object({ metadata: z.object({ owner: z.string().nullish() }).nullish() })).safeParse(await res.json()).data?.[0];
  if (!entity) return "missing";
  return entity.metadata?.owner ? loadCreator(entity.metadata.owner, opts) : undefined;
}

async function loadPlaces(path: string, query: Record<string, string>, opts: GetOptions): Promise<Place[]> {
  return Places.parse(await getJSON<unknown>(placesApiPath("get", path as "/api/places"), { ...opts, query })).data ?? [];
}

/** The places record for a jump: a World's scene by name and position, a World by name, or the parcel. */
async function loadJumpPlaces({ placesPosition: at, realm }: JumpTarget, opts: GetOptions): Promise<Place[]> {
  const positions = at?.join(",");
  if (realm && isEns(realm)) {
    const names = realm.toLowerCase();
    if (!positions) return loadPlaces("/api/worlds", { names }, opts);
    const places = await loadPlaces("/api/places", { names, positions }, opts);
    // That record's user_count is the Genesis City parcel's: the World's own count comes from /worlds.
    const world = places[0] ? await loadPlaces("/api/worlds", { names }, opts).catch(() => null) : null;
    const count = world?.[0]?.user_count;
    return count != null ? [{ ...places[0]!, user_count: count }, ...places.slice(1)] : places;
  }
  return loadPlaces("/api/places", positions ? { positions } : {}, opts);
}

function jumpHref(coordinates: [number, number], realm?: string): string {
  return playUrl({ position: coordinates.join(","), realm });
}

function placeCard(place: Place | undefined, target: JumpTarget, creator: Creator | undefined): JumpCardData {
  if (!place) {
    return { type: "place", title: target.realm ?? "", userName: "Unknown", coordinates: target.position.coordinates, userCount: 0, realm: target.realm, playHref: jumpHref(target.position.coordinates, target.realm) };
  }
  const coordinates = place.base_position.split(",").map(Number) as [number, number];
  const owner = place.owner && ADDRESS.test(place.owner) ? place.owner : undefined;
  const realm = place.world ? place.world_name ?? undefined : undefined;
  return {
    type: "place",
    title: place.title ?? "",
    userName: creator?.name ?? (place.contact_name || (owner ? undefined : place.owner) || "Unknown"),
    user: creator?.user ?? owner,
    avatar: creator?.avatar,
    foundation: place.contact_name === FOUNDATION_CONTACT_NAME,
    avatarColor: creator?.color,
    coordinates,
    image: place.image ?? undefined,
    description: place.description ?? undefined,
    userCount: place.user_count ?? 0,
    realm,
    playHref: jumpHref(coordinates, realm),
  };
}

export type JumpPlaceResult = { card: JumpCardData } | { invalid: true };

/** The card for /jump and /jump/places; invalid when the position is not one, the places API fails or the World is unknown. */
export async function loadJumpPlace(target: JumpTarget, opts: GetOptions = {}): Promise<JumpPlaceResult> {
  if (!target.position.isValid) return { invalid: true };
  const world = target.realm && isEns(target.realm) ? target.realm : undefined;
  const [places, creator] = await Promise.all([
    loadJumpPlaces(target, opts).catch(() => null),
    (world ? loadWorldOwner(world, opts) : loadDeployer(target.position.coordinates.join(","), opts)).catch(() => undefined),
  ]);
  if (!places || creator === "missing") return { invalid: true };
  return { card: placeCard(places[0], target, creator) };
}

export type JumpEventResult = { card: JumpCardData; deleted: boolean; event: Event } | { invalid: true };

/** The card for /jump/events: the event by `?id=`, else the first at the position; invalid when there is none. */
export async function loadJumpEvent(target: JumpTarget, opts: GetOptions = {}): Promise<JumpEventResult> {
  if (!target.id && !target.position.isValid) return { invalid: true };
  let event: Event | undefined;
  try {
    if (target.id) {
      const raw = await getJSON<{ ok?: boolean; data?: unknown }>(eventsApiPath("get", "/api/events/{event_id}", { event_id: target.id }), opts);
      event = raw.ok === false ? undefined : Event.parse(raw.data);
    } else {
      const query: Record<string, string> = { position: target.position.coordinates.join(",") };
      if (target.realm) query["world_names[]"] = target.realm;
      const raw = await getJSON<{ data?: unknown[] }>(eventsApiPath("get", "/api/events"), { ...opts, query });
      event = raw.data?.[0] ? Event.parse(raw.data[0]) : undefined;
    }
  } catch {
    return { invalid: true };
  }
  if (!event) return { invalid: true };
  const deleted = !!(event.deleted_by_user || event.deleted_by_admin);
  const creator = event.user ? await loadCreator(event.user, opts).catch(() => undefined) : undefined;
  const realm = event.world && event.server ? event.server : undefined;
  let userCount = 0;
  if (event.live && !deleted) {
    const places = await loadJumpPlaces(target, opts).catch(() => null);
    const match = places?.find((p) => p.title === event.scene_name || p.base_position === event.coordinates.join(","));
    userCount = match?.user_count ?? 0;
  }
  return {
    deleted,
    event,
    card: {
      type: "event",
      title: event.name,
      userName: creator?.name ?? event.user_name ?? "",
      user: event.user ?? undefined,
      avatar: creator?.avatar,
      avatarColor: creator?.color,
      coordinates: event.coordinates,
      image: event.image ?? undefined,
      description: event.description ?? undefined,
      userCount,
      realm,
      playHref: jumpHref(event.coordinates, realm),
      startAt: event.next_start_at,
      // A deleted event shows neither as live nor as ended.
      finishAt: deleted ? undefined : event.next_finish_at,
      live: !deleted && !!event.live,
      totalAttendees: event.total_attendees ?? 0,
    },
  };
}
