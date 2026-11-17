// The full map's layers as the minimap draws them: positions in world metres, which the minimap's
// own atlas layout takes, and when the route's destination leaves the map for the minimap's edge.

import { PARCEL_METERS } from '../map/atlas'
import { PARCEL_UNITS, type Vec2 } from '../map/mapCoords'

/** A full-map position (map units, y up) in world metres. */
export function mapUnitsToMeters(pos: Vec2): { x: number; z: number } {
  return { x: (pos.x / PARCEL_UNITS + 1) * PARCEL_METERS, z: (pos.y / PARCEL_UNITS + 1) * PARCEL_METERS }
}

/** Squared map units (about eight parcels): nearer than this the route's destination is left to the
 *  minimap itself, and its pin leaves the minimap's edge. */
export const EDGE_PIN_HIDE_SQUARED = 26000

/** Whether the destination is far enough that its pin stands on the minimap's edge. */
export function destinationOnEdge(player: Vec2, destination: Vec2): boolean {
  const dx = destination.x - player.x
  const dy = destination.y - player.y
  return dx * dx + dy * dy > EDGE_PIN_HIDE_SQUARED
}

/**
 * Where on the edge of the square minimap a direction points, as a fraction of the way from the
 * centre to the frame (the compass badges' rule): `dx` to the right and `dy` down, in screen px.
 */
export function edgePoint(dx: number, dy: number): { x: number; y: number } {
  const reach = Math.max(Math.abs(dx), Math.abs(dy))
  return reach === 0 ? { x: 0, y: 0 } : { x: dx / reach, y: dy / reach }
}
