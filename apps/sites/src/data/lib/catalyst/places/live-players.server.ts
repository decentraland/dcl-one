import { getJSON } from "../client";
import type { Place } from "../schema";
import { LiveDataSchema, WCS_LIVE_DATA_PATH, wcsBase } from "../wcs";

type HotScene = { baseCoords?: unknown; usersTotalCount?: unknown };

/** The node's hot scenes, in the shape decentraland.org reads from the realm provider. */
export const HOT_SCENES_PATH = "/archipelago-ea-stats/hot-scenes";

/** Players in a place now, as decentraland.org gates its live view: a hot scene counts only
    when its base parcel is one of the place's parcels; a world reads the worlds live data.
    A read failure is unknown (null); the page then shows the jump-in card. */
export async function loadLivePlayers(place: Place, opts: { signal?: AbortSignal } = {}): Promise<number | null> {
  try {
    if (place.world) {
      // As decentraland.org counts them: the live data's own count, whatever its observation time.
      const live = LiveDataSchema.parse(await getJSON<unknown>(WCS_LIVE_DATA_PATH, { base: wcsBase(), signal: opts.signal }));
      const name = place.world_name?.toLowerCase();
      return live.data.perWorld.find((w) => w.worldName.toLowerCase() === name)?.users ?? 0;
    }
    const scenes = await getJSON<unknown>(HOT_SCENES_PATH, { query: { limit: 40 }, signal: opts.signal });
    if (!Array.isArray(scenes)) return 0;
    const parcels = new Set([...place.positions, place.base_position]);
    const scene = (scenes as HotScene[]).find(
      (s) => Array.isArray(s.baseCoords) && parcels.has(`${s.baseCoords[0]},${s.baseCoords[1]}`),
    );
    return typeof scene?.usersTotalCount === "number" ? scene.usersTotalCount : 0;
  } catch {
    return null;
  }
}

/** decentraland.org never mounts the live viewer on a phone; the request's user agent says which one this is. */
export function isMobileRequest(request: Request): boolean {
  return /Mobi|Android|iPhone|iPad|iPod/i.test(request.headers.get("user-agent") ?? "");
}
