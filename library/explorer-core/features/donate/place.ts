// The place Donate names for where the player stands, from the engine's `sceneInfo`: a Genesis City parcel (the
// places service finds the place covering it), or a World by its name. A preview, a local realm or no scene
// gives none, so Donate doesn't show there.
import type { DonatePlace } from '../../hostContract'

/** A World's name in a realm: the realm itself, or its URL's last path segment, ending in `.eth`. */
const WORLD = /(?:^|\/)([a-z0-9][a-z0-9.-]*\.eth)\/?$/i

export function donatePlaceOf(info: { parcel?: { x: number; y: number }; realm?: string; genesis?: boolean; preview?: boolean }): DonatePlace | null {
  if (info.preview) return null
  if (info.genesis) return info.parcel ? { parcel: { x: info.parcel.x, y: info.parcel.y } } : null
  const world = info.realm?.match(WORLD)?.[1]
  return world ? { world: world.toLowerCase() } : null
}
