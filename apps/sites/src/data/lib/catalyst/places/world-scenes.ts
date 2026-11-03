import { z } from "zod";
import { playUrl } from "../bases";
import { getJSON, type GetOptions } from "../client";
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

/** Resolve the deployment's launch target once on the server, including its realm. */
export async function loadPlaceTravel(place: { world: boolean; world_name: string | null; base_position: string }, options: GetOptions = {}): Promise<PlaceTravel> {
  const playBase = playUrl();
  if (place.world && place.world_name) {
    return { playBase, jumpHref: sceneTravelHref(place.world_name, place.base_position, playBase) };
  }
  try {
    const timeout = AbortSignal.timeout(3000);
    const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;
    const about = z.object({ configurations: z.object({ realmName: z.string().trim().min(1) }) })
      .parse(await getJSON("/about", { ...options, signal }));
    return { playBase, jumpHref: sceneTravelHref(about.configurations.realmName, place.base_position, playBase) };
  } catch (error) {
    if (options.signal?.aborted) throw error;
    return { playBase, launchError: "Travel is temporarily unavailable. Please retry." };
  }
}
