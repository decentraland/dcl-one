// The full map's layers as the minimap draws them: positions in world metres, which the minimap's
// own atlas layout takes, and when and where the route's destination stands on the minimap's edge.

import { PARCEL_METERS } from '../map/atlas'
import { PARCEL_UNITS, type Vec2 } from '../map/mapCoords'

/** A full-map position (map units, y up) in world metres. */
export function mapUnitsToMeters(pos: Vec2): { x: number; z: number } {
  return { x: (pos.x / PARCEL_UNITS + 1) * PARCEL_METERS, z: (pos.y / PARCEL_UNITS + 1) * PARCEL_METERS }
}

/** Whether a point `dx`, `dy` screen px from the centre of the square minimap, `half` px from
 *  centre to edge, lies off it: then its pin stands on the edge instead. */
export function offMap(dx: number, dy: number, half: number): boolean {
  return Math.max(Math.abs(dx), Math.abs(dy)) > half
}

/**
 * Where on the edge of the square minimap a direction points, as a fraction of the way from the
 * centre to the frame (the compass badges' rule): `dx` to the right and `dy` down, in screen px.
 */
export function edgePoint(dx: number, dy: number): { x: number; y: number } {
  const reach = Math.max(Math.abs(dx), Math.abs(dy))
  return reach === 0 ? { x: 0, y: 0 } : { x: dx / reach, y: dy / reach }
}
