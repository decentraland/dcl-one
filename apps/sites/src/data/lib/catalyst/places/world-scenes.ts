import { z } from "zod";
import { playUrl } from "../bases";
import { catalystBase, getJSON, type GetOptions } from "../client";
import { ApiDataTotalSchema } from "../generated-schemas/places";

const WorldScene = z.object({ id: z.string(), title: z.string().nullable(), base_position: z.string(), world: z.boolean(), world_name: z.string().nullable() });
export type WorldScene = z.infer<typeof WorldScene>;

export async function loadWorldScenes(worldName: string, offset: number, options: GetOptions = {}) {
  const response = ApiDataTotalSchema(WorldScene).parse(await getJSON(`/places/api/worlds/${encodeURIComponent(worldName.toLowerCase())}/scenes`, { ...options, query: { offset, limit: 20 } }));
  return { scenes: response.data, total: response.total };
}

export function sceneTravelHref(worldName: string | null | undefined, position: string, entry?: string): string {
  const query = new URLSearchParams();
  if (worldName) query.set("realm", worldName);
  if (/^-?\d+,-?\d+$/.test(position)) query.set("position", position);
  return entry === undefined ? playUrl(query) : `${entry}?${query}`;
}

export type PlaceTravel = { playBase: string; jumpHref?: string; launchError?: string };

const REALM_TTL_MS = 5 * 60_000;
const REALM_CACHE_LIMIT = 8;
const realms = new Map<string, { name: string; expires: number }>();
const AboutRealm = z.object({ configurations: z.object({ realmName: z.string().trim().min(1) }) });

async function loadRealm(options: GetOptions): Promise<string> {
  options.signal?.throwIfAborted();
  const key = catalystBase(options.base);
  const cacheable = !options.fetchImpl;
  const cached = cacheable ? realms.get(key) : undefined;
  if (cached && cached.expires > Date.now()) return cached.name;
  const timeout = AbortSignal.timeout(3000);
  const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;
  const about = AboutRealm.parse(await getJSON("/about", { ...options, signal }));
  const name = about.configurations.realmName;
  if (cacheable) {
    realms.delete(key);
    realms.set(key, { name, expires: Date.now() + REALM_TTL_MS });
    while (realms.size > REALM_CACHE_LIMIT) realms.delete(realms.keys().next().value!);
  }
  return name;
}

/** Resolve a real launch target; null leaves the route to present a retryable failure. */
export async function loadPlaceTravel(place: { world: boolean; world_name: string | null; base_position: string }, options: GetOptions = {}): Promise<PlaceTravel | null> {
  const playBase = playUrl();
  if (place.world && place.world_name) {
    return { playBase, jumpHref: sceneTravelHref(place.world_name, place.base_position, playBase) };
  }
  try {
    const realm = await loadRealm(options);
    return { playBase, jumpHref: sceneTravelHref(realm, place.base_position, playBase) };
  } catch (error) {
    if (options.signal?.aborted) throw error;
    return null;
  }
}
