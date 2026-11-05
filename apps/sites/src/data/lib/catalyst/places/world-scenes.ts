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
const REALM_FAILURE_TTL_MS = 10_000;
const REALM_CACHE_LIMIT = 8;
const realms = new Map<string, { name: string | null; expires: number }>();
const pendingRealms = new Map<string, Promise<string | null>>();
const AboutRealm = z.object({ configurations: z.object({ realmName: z.string().trim().min(1) }) });
type TravelOptions = GetOptions & { refreshRealm?: boolean };

function rememberRealm(key: string, name: string | null) {
  realms.delete(key);
  realms.set(key, { name, expires: Date.now() + (name === null ? REALM_FAILURE_TTL_MS : REALM_TTL_MS) });
  while (realms.size > REALM_CACHE_LIMIT) {
    const oldest = realms.keys().next().value;
    if (oldest !== undefined) realms.delete(oldest);
  }
}

function waitForRealm(read: Promise<string | null>, signal?: AbortSignal): Promise<string | null> {
  if (!signal) return read;
  signal.throwIfAborted();
  return new Promise((resolve, reject) => {
    const aborted = () => { signal.removeEventListener("abort", aborted); reject(signal.reason); };
    signal.addEventListener("abort", aborted, { once: true });
    read.then(
      (name) => { signal.removeEventListener("abort", aborted); resolve(name); },
      (error) => { signal.removeEventListener("abort", aborted); reject(error); },
    );
  });
}

async function readRealm(options: GetOptions): Promise<string> {
  const about = AboutRealm.parse(await getJSON("/about", options));
  return about.configurations.realmName;
}

async function loadRealm(options: TravelOptions): Promise<string | null> {
  options.signal?.throwIfAborted();
  // Custom fetchers/headers are scoped reads, outside the public node cache.
  if (options.fetchImpl || options.headers || options.query) {
    const timeout = AbortSignal.timeout(3000);
    const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;
    return readRealm({ ...options, signal });
  }
  const key = catalystBase(options.base);
  const cached = realms.get(key);
  if (!options.refreshRealm && cached && cached.expires > Date.now()) return cached.name;
  let read = pendingRealms.get(key);
  if (!read) {
    if (pendingRealms.size >= REALM_CACHE_LIMIT) throw new Error("Realm lookup capacity is busy.");
    read = (async () => {
      try {
        const name = await readRealm({ ...options, signal: AbortSignal.timeout(3000) });
        rememberRealm(key, name);
        return name;
      } catch {
        rememberRealm(key, null);
        return null;
      } finally {
        pendingRealms.delete(key);
      }
    })();
    pendingRealms.set(key, read);
  }
  return waitForRealm(read, options.signal);
}

/** Resolve a real launch target; null leaves the route to present a retryable failure. */
export async function loadPlaceTravel(place: { world: boolean; world_name: string | null; base_position: string }, options: TravelOptions = {}): Promise<PlaceTravel | null> {
  options.signal?.throwIfAborted();
  const playBase = playUrl();
  if (place.world && place.world_name) {
    return { playBase, jumpHref: sceneTravelHref(place.world_name, place.base_position, playBase) };
  }
  try {
    const realm = await loadRealm(options);
    return realm === null ? null : { playBase, jumpHref: sceneTravelHref(realm, place.base_position, playBase) };
  } catch (error) {
    if (options.signal?.aborted) throw error;
    return null;
  }
}
