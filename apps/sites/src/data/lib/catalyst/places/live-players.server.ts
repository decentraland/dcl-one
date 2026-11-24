import { getJSON } from "../client";
import type { Place } from "../schema";
import { LiveDataSchema, WCS_LIVE_DATA_PATH, wcsBase } from "../wcs";
import { HOT_SCENES_PATH, HotScene, hotSceneRows } from "./hot-scenes";

/** The count needs no title: a scene without one still has its players. */
const SceneCount = HotScene.pick({ baseCoords: true, usersTotalCount: true });

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
    const raw = await getJSON<unknown>(HOT_SCENES_PATH, { signal: opts.signal });
    if (!Array.isArray(raw)) return 0;
    const parcels = new Set([...place.positions, place.base_position]);
    const scene = hotSceneRows(SceneCount, raw).find((s) => parcels.has(`${s.baseCoords[0]},${s.baseCoords[1]}`));
    return scene?.usersTotalCount ?? 0;
  } catch {
    return null;
  }
}

/** decentraland.org never mounts the live viewer on a phone; the request's user agent says which one this is. */
export function isMobileRequest(request: Request): boolean {
  return /Mobi|Android|iPhone|iPad|iPod/i.test(request.headers.get("user-agent") ?? "");
}
