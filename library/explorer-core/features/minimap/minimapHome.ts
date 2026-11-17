// The minimap menu's Set as Home: home is the World the player is in, or the Genesis City parcel
// they stand on. Whether the player stands in their home is `isStandingInHome` (../map/home).

import { DEFAULT_REALM } from '@hud-host'
import type { Vec2 } from '../map/mapCoords'

/** The home Set as Home makes: a World by its name, or the parcel the player stands on. */
export function minimapHomeTarget(linkRealm: string, parcel: Vec2): { realm: string; parcel: Vec2 } {
  return linkRealm === DEFAULT_REALM ? { realm: DEFAULT_REALM, parcel } : { realm: linkRealm, parcel: { x: 0, y: 0 } }
}
