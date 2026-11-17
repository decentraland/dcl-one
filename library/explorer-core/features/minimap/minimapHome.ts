// The minimap menu's Set as Home: home is the World the player is in, or the Genesis City parcel
// they stand on, and the switch reads on wherever the player already stands in their home.

import { DEFAULT_REALM } from '@hud-host'
import type { DiscoverPlace } from '../places/placesApi'
import { isHomePlace, type Home } from '../map/home'
import { isRoad } from '../map/mapApi'
import type { Vec2 } from '../map/mapCoords'

/** The home Set as Home makes: a World by its name, or the parcel the player stands on. */
export function minimapHomeTarget(linkRealm: string, parcel: Vec2): { realm: string; parcel: Vec2 } {
  return linkRealm === DEFAULT_REALM ? { realm: DEFAULT_REALM, parcel } : { realm: linkRealm, parcel: { x: 0, y: 0 } }
}

/**
 * Whether the player stands in their home. In a World, by the World's name. In Genesis City, home
 * is one parcel, so any parcel of the place under the player counts; a road spans the city, so it
 * and a parcel with no place count only on the home parcel itself.
 */
export function isStandingInHome(home: Home, linkRealm: string, parcel: Vec2, place: DiscoverPlace | null): boolean {
  if (linkRealm !== DEFAULT_REALM) return home.world != null && home.world.toLowerCase() === linkRealm.toLowerCase()
  if (place != null && !isRoad(place.title)) return isHomePlace(home, place)
  return home.world == null && home.parcel != null && home.parcel.x === parcel.x && home.parcel.y === parcel.y
}
