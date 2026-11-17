import { queryOptions } from "@tanstack/react-query";

import { getJSON } from "../catalyst/client";
import { HOT_SCENES_PATH, HotScene, hotSceneRows } from "../catalyst/places/hot-scenes";

export type { HotScene };

// The /create rail has six card places: it shows the six scenes with the most players.
const MAX_SCENES = 6;

/** The busiest occupied scenes, Genesis Plaza left out (decentraland.org /create: LiveScenes). */
export function selectLiveScenes(scenes: readonly HotScene[]): HotScene[] {
  return scenes
    .filter((s) => !s.name.toLowerCase().includes("genesis plaza") && s.usersTotalCount > 0)
    .sort((a, b) => b.usersTotalCount - a.usersTotalCount)
    .slice(0, MAX_SCENES);
}

/**
 * The node's occupied scenes, each row checked: a row that is not a scene card is left out. An unavailable feed
 * fails the read: the query has no data and the section stays hidden.
 */
export async function fetchHotScenes(signal?: AbortSignal): Promise<HotScene[]> {
  return hotSceneRows(HotScene, await getJSON<unknown>(HOT_SCENES_PATH, { signal }));
}

export const liveScenesQuery = () =>
  queryOptions({
    queryKey: ["places", "hot-scenes"] as const,
    queryFn: ({ signal }) => fetchHotScenes(signal),
    staleTime: 60_000,
  });
