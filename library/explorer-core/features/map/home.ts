// The player's home as the map reads it: one parcel of Genesis City, or a world.

import type { DiscoverPlace } from '../places/placesApi'
import { isRoad, isWorldPlace } from './mapApi'
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

/** Whether home is this Genesis City parcel. */
export function isHomeParcel(home: Home, parcel: Vec2 | null): boolean {
  return home.world == null && parcel != null && home.parcel != null && home.parcel.x === parcel.x && home.parcel.y === parcel.y
}

/**
 * The two places that say whether something is home, side by side so they can't drift apart:
 * - the map's place panel: the place is home when any parcel of it is, and a road also when the
 *   parcel it was picked at is home; a World's name compares exactly;
 * - the minimap's Set as Home: the place under the player is home when any parcel of it is, but a
 *   road, which spans the city, or a parcel with no place, only on the home parcel itself; a
 *   World's name compares without case.
 */
export function isPlacePanelHome(home: Home, place: DiscoverPlace, origin: Vec2 | null): boolean {
  return isHomePlace(home, place) || (isRoad(place.title) && isHomeParcel(home, origin))
}

/** See `isPlacePanelHome`. `world` is the World the player is in, or null in Genesis City. */
export function isStandingInHome(home: Home, world: string | null, parcel: Vec2, place: DiscoverPlace | null): boolean {
  if (world != null) return home.world != null && home.world.toLowerCase() === world.toLowerCase()
  if (place != null && !isRoad(place.title)) return isHomePlace(home, place)
  return isHomeParcel(home, parcel)
}
