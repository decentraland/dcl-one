import { queryOptions } from "@tanstack/react-query";

import { catalystUrl } from "../catalyst/bases";

/** One row of the node's `/hot-scenes` (the realm provider's shape). */
export type HotScene = {
  id: string;
  name: string;
  baseCoords: [number, number];
  usersTotalCount: number;
  parcels: [number, number][];
  thumbnail: string;
};

const MAX_SCENES = 6;

/** The busiest occupied scenes, Genesis Plaza left out (decentraland.org /create: LiveScenes). */
export function selectLiveScenes(scenes: readonly HotScene[]): HotScene[] {
  return scenes
    .filter((s) => !s.name.toLowerCase().includes("genesis plaza") && s.usersTotalCount > 0)
    .sort((a, b) => b.usersTotalCount - a.usersTotalCount)
    .slice(0, MAX_SCENES);
}

/** The node's occupied scenes; an unavailable feed is no scenes, so the section stays hidden. */
export async function fetchHotScenes(signal?: AbortSignal): Promise<HotScene[]> {
  const res = await fetch(`${catalystUrl()}/hot-scenes`, { signal, headers: { accept: "application/json" } });
  if (!res.ok) return [];
  const body: unknown = await res.json();
  return Array.isArray(body) ? (body as HotScene[]) : [];
}

export const liveScenesQuery = () =>
  queryOptions({
    queryKey: ["public", "hot-scenes"] as const,
    queryFn: ({ signal }) => fetchHotScenes(signal),
    staleTime: 60_000,
  });
