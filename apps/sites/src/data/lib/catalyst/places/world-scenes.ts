import { z } from "zod";
import { getJSON, type GetOptions } from "../client";
import { ApiDataTotalSchema } from "../generated-schemas/places";

const WorldScene = z.object({ id: z.string(), title: z.string().nullable(), base_position: z.string(), world: z.boolean(), world_name: z.string().nullable() });
export type WorldScene = z.infer<typeof WorldScene>;

export async function loadWorldScenes(worldName: string, offset: number, options: GetOptions = {}) {
  const response = ApiDataTotalSchema(WorldScene).parse(await getJSON(`/places/api/worlds/${encodeURIComponent(worldName.toLowerCase())}/scenes`, { ...options, query: { offset, limit: 20 } }));
  return { scenes: response.data, total: response.total };
}

export function sceneTravelHref(worldName: string | null | undefined, position: string): string {
  const query = new URLSearchParams();
  if (worldName) query.set("realm", worldName);
  if (/^-?\d+,-?\d+$/.test(position)) query.set("position", position);
  return `/play/?${query}`;
}
