import { z } from "zod";

import { CatalystError, catalystBase, getJSON, worldsBase, type GetOptions } from "../client";
import { EventRecordSchema } from "../generated-schemas/events";
import { eventsApiPath } from "../typed";
import { fetchProfile } from "../social/profile";
import { isEnsRealm, type JumpEventRecord, type JumpPlaceRecord } from "./jump";

const PlaceRow = z.object({
  title: z.string().nullable().catch(null),
  description: z.string().nullable().catch(null),
  image: z.string().nullable().catch(null),
  base_position: z.string(),
  owner: z.string().nullable().catch(null),
  contact_name: z.string().nullable().catch(null),
  user_count: z.number().nullable().catch(null),
  world: z.boolean().catch(false),
  world_name: z.string().nullable().catch(null),
});
const PlaceList = z.object({ data: z.array(z.unknown()) });

function parsePlaces(raw: unknown): JumpPlaceRecord[] {
  const rows: JumpPlaceRecord[] = [];
  for (const row of PlaceList.parse(raw).data) {
    const parsed = PlaceRow.safeParse(row);
    if (parsed.success) rows.push(parsed.data);
  }
  return rows;
}

/**
 * A World link with a position asks for that World's scene; without one, the World-level record.
 * `/places?names=&positions=` reports the Genesis City parcel's occupancy, so the World's own count
 * from `/worlds` replaces it.
 */
export async function loadJumpPlace(
  target: { lookup: [number, number] | undefined; realm: string | undefined },
  opts: GetOptions = {},
): Promise<JumpPlaceRecord | null> {
  const { lookup, realm } = target;
  const positions = lookup ? lookup.join(",") : undefined;
  if (realm && isEnsRealm(realm)) {
    const names = realm.toLowerCase();
    if (!positions) {
      return parsePlaces(await getJSON("/places/api/worlds", { ...opts, query: { names } }))[0] ?? null;
    }
    const [scene] = parsePlaces(await getJSON("/places/api/places", { ...opts, query: { names, positions } }));
    if (!scene) return null;
    const world = await getJSON("/places/api/worlds", { ...opts, query: { names } })
      .then((raw) => parsePlaces(raw)[0] ?? null)
      .catch(() => null);
    return world?.user_count != null ? { ...scene, user_count: world.user_count } : scene;
  }
  return parsePlaces(await getJSON("/places/api/places", { ...opts, query: { positions } }))[0] ?? null;
}

const ActiveEntities = z.array(
  z.object({ id: z.string(), metadata: z.object({ owner: z.string().optional() }).passthrough().optional() }).passthrough(),
);
type ActiveEntity = z.infer<typeof ActiveEntities>[number];

async function postActiveEntity(url: string, pointer: string, opts: GetOptions): Promise<ActiveEntity | null> {
  const res = await (opts.fetchImpl ?? fetch)(url, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json" },
    body: JSON.stringify({ pointers: [pointer] }),
    signal: opts.signal,
  });
  if (!res.ok) {
    await res.body?.cancel();
    throw new CatalystError(`Active entity lookup answered ${res.status}`, url, res.status);
  }
  return ActiveEntities.parse(await res.json())[0] ?? null;
}

/** False only when the worlds server answers that the name has no active scene; an outage throws. */
export async function worldExists(realm: string, opts: GetOptions = {}): Promise<boolean> {
  const entity = await postActiveEntity(`${worldsBase(opts.base)}/entities/active`, realm.toLowerCase(), opts);
  return entity !== null;
}

export type JumpCreator = { address: string; name: string; avatar?: string };

export async function loadCreator(address: string, opts: GetOptions): Promise<JumpCreator | null> {
  const avatar = await fetchProfile(address, opts);
  const name = avatar?.name || avatar?.realName;
  if (!name) return null;
  return { address, name, avatar: avatar?.avatar?.snapshots?.face256 };
}

const Deployments = z.object({ deployments: z.array(z.object({ deployedBy: z.string() }).passthrough()) });

/**
 * Who deployed the scene: a World's owner from its active entity, a parcel's deployer from its deployment.
 * Null when there is no scene or the wallet has no profile name, so the card keeps the catalogue's contact name.
 */
export async function loadSceneCreator(
  target: { coordinates: [number, number]; realm: string | undefined },
  opts: GetOptions = {},
): Promise<JumpCreator | null> {
  if (target.realm && isEnsRealm(target.realm)) {
    const entity = await postActiveEntity(`${worldsBase(opts.base)}/entities/active`, target.realm.toLowerCase(), opts);
    const owner = entity?.metadata?.owner;
    return owner ? loadCreator(owner, opts) : null;
  }
  const entity = await postActiveEntity(`${catalystBase(opts.base)}/content/entities/active`, target.coordinates.join(","), opts);
  if (!entity) return null;
  const raw = await getJSON("/content/deployments", { ...opts, query: { entityId: entity.id } });
  const deployedBy = Deployments.parse(raw).deployments[0]?.deployedBy;
  return deployedBy ? loadCreator(deployedBy, opts) : null;
}

const JumpEventRow = EventRecordSchema.pick({
  id: true,
  name: true,
  image: true,
  description: true,
  next_start_at: true,
  next_finish_at: true,
  coordinates: true,
  user: true,
  user_name: true,
  total_attendees: true,
  live: true,
  world: true,
  server: true,
  deleted_by_user: true,
  deleted_by_admin: true,
});
const EventEnvelope = z.object({ ok: z.boolean().optional(), data: z.unknown() });

/** The event a link names: by id, else the first one at the link's parcel or World. Null when none. */
export async function loadJumpEvent(
  target: { id: string | null; coordinates: [number, number]; realm: string | undefined },
  opts: GetOptions = {},
): Promise<JumpEventRecord | null> {
  if (target.id) {
    try {
      const raw = await getJSON(eventsApiPath("get", "/api/events/{event_id}", { event_id: target.id }), opts);
      const parsed = JumpEventRow.safeParse(EventEnvelope.parse(raw).data);
      return parsed.success ? parsed.data : null;
    } catch (error) {
      if (error instanceof CatalystError && error.status === 404) return null;
      throw error;
    }
  }
  const query: Record<string, string> = { position: target.coordinates.join(",") };
  if (target.realm) query["world_names[]"] = target.realm;
  const raw = await getJSON(eventsApiPath("get", "/api/events"), { ...opts, query });
  const data = EventEnvelope.parse(raw).data;
  const rows = Array.isArray(data) ? data : z.object({ events: z.array(z.unknown()) }).parse(data).events;
  for (const row of rows) {
    const parsed = JumpEventRow.safeParse(row);
    if (parsed.success) return parsed.data;
  }
  return null;
}
