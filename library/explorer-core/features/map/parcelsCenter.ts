import type { Vec2 } from './mapCoords'

/**
 * The parcel a multi-parcel place's marker is drawn on. It walks the parcels in order, starting
 * on the first, and moves to a parcel when that parcel is nearer the average than the distance
 * last recorded. That distance is the step from the previously chosen parcel to the newly chosen
 * one (from the origin, for the first), not a distance to the average. The result therefore
 * depends on parcel order and is not always the parcel nearest the middle.
 */
export function parcelsCenter(positions: readonly Vec2[]): Vec2 {
  let cx = 0
  let cy = 0
  for (const p of positions) {
    cx += p.x
    cy += p.y
  }
  cx /= positions.length
  cy /= positions.length

  let distance = Number.POSITIVE_INFINITY
  let center: Vec2 = { x: 0, y: 0 }
  for (const parcel of positions) {
    if (Math.hypot(cx - parcel.x, cy - parcel.y) < distance) {
      distance = Math.hypot(center.x - parcel.x, center.y - parcel.y)
      center = parcel
    }
  }
  return center
}
