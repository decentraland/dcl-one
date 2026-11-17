// The player's home as the map reads it: one parcel of Genesis City, or a world.

import type { DiscoverPlace } from '../places/placesApi'
import { isWorldPlace } from './mapApi'
import { parseParcel, type Vec2 } from './mapCoords'

export interface Home {
  /** The home parcel, when home is in Genesis City. */
  parcel: Vec2 | null
  /** The home world's name, when home is a world. */
  world: string | null
}

export const NO_HOME: Home = { parcel: null, world: null }

/** The home the session holds: a realm (null for the default one) and an `x,y` parcel. */
export function homeOf(scene: { realm: string | null; parcel: string } | null): Home {
  if (scene == null) return NO_HOME
  if (scene.realm != null) return { parcel: null, world: scene.realm }
  return { parcel: parseParcel(scene.parcel), world: null }
}

/** Home is one parcel, so any parcel of a place makes the whole place home. */
export function isHomePlace(home: Home, place: DiscoverPlace): boolean {
  if (isWorldPlace(place)) return home.world === place.world_name
  const at = home.parcel
  if (home.world != null || at == null) return false
  if (place.positions.length > 0) return place.positions.some((p) => p === `${at.x},${at.y}`)
  const base = parseParcel(place.base_position)
  return base != null && base.x === at.x && base.y === at.y
}
