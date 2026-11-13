// Map space for the full-screen map: 20 map units per parcel, y up. Parcel (x, y) covers the
// half-open square (20x-20, 20x] × (20y-20, 20y], so its centre is (20x-10, 20y-10) and a
// position maps back to its parcel by rounding up.

import { PARCEL_METERS } from './atlas'

export const PARCEL_UNITS = 20

export interface Vec2 {
  x: number
  y: number
}

export function coordsToPosition(parcel: Vec2): Vec2 {
  return { x: parcel.x * PARCEL_UNITS, y: parcel.y * PARCEL_UNITS }
}

export function parcelCenter(parcel: Vec2): Vec2 {
  return { x: parcel.x * PARCEL_UNITS - PARCEL_UNITS / 2, y: parcel.y * PARCEL_UNITS - PARCEL_UNITS / 2 }
}

export function positionToCoords(pos: Vec2): Vec2 {
  return { x: Math.ceil(pos.x / PARCEL_UNITS), y: Math.ceil(pos.y / PARCEL_UNITS) }
}

export function worldToGrid(worldX: number, worldZ: number): Vec2 {
  return { x: worldX / PARCEL_METERS, y: worldZ / PARCEL_METERS }
}

export function playerMarkerPosition(worldX: number, worldZ: number): Vec2 {
  const grid = worldToGrid(worldX, worldZ)
  // the centre offset and the marker's half-parcel pivot add up to one whole parcel
  return { x: grid.x * PARCEL_UNITS - PARCEL_UNITS, y: grid.y * PARCEL_UNITS - PARCEL_UNITS }
}

export const userMarkerPosition = playerMarkerPosition

export function parseParcel(text: string | undefined | null): Vec2 | null {
  if (text == null) return null
  const [xs, ys] = text.split(',')
  if (ys === undefined) return null
  const x = Number(xs)
  const y = Number(ys)
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null
}

export const parcelKey = (p: Vec2): string => `${p.x},${p.y}`

const MIN_PARCEL: Vec2 = { x: -150, y: -150 }
const MAX_PARCEL: Vec2 = { x: 163, y: 158 }
const MAX_SQUARE_CITY_PARCEL: Vec2 = { x: 150, y: 150 }
const PADDING = 25

const WORLD_MAX_COORDS: Vec2 = { x: MAX_SQUARE_CITY_PARCEL.x + PADDING, y: MAX_SQUARE_CITY_PARCEL.y + PADDING }

export interface Rect {
  xMin: number
  yMin: number
  xMax: number
  yMax: number
}

export const VISIBLE_WORLD_BOUNDS: Rect = {
  xMin: (MIN_PARCEL.x - PADDING - 1) * PARCEL_UNITS,
  yMin: (MIN_PARCEL.y - PADDING - 1) * PARCEL_UNITS,
  xMax: WORLD_MAX_COORDS.x * PARCEL_UNITS,
  yMax: WORLD_MAX_COORDS.y * PARCEL_UNITS
}

// The city is not square: two strips of extra parcels sit along its north and east edges.
const INTERACTABLE_PARCEL_RECTS: Rect[] = [
  { xMin: MIN_PARCEL.x, yMin: MIN_PARCEL.y, xMax: MAX_SQUARE_CITY_PARCEL.x, yMax: MAX_SQUARE_CITY_PARCEL.y },
  { xMin: 62, yMin: 151, xMax: 162, yMax: MAX_PARCEL.y },
  { xMin: 151, yMin: 59, xMax: MAX_PARCEL.x, yMax: 150 }
]
const INTERACTABLE_BOUNDS: Rect[] = INTERACTABLE_PARCEL_RECTS.map((r) => ({
  xMin: (r.xMin - 1) * PARCEL_UNITS,
  yMin: (r.yMin - 1) * PARCEL_UNITS,
  xMax: r.xMax * PARCEL_UNITS,
  yMax: r.yMax * PARCEL_UNITS
}))

export function parcelWithinBounds(pos: Vec2): Vec2 | null {
  for (const r of INTERACTABLE_BOUNDS) {
    if (pos.x >= r.xMin && pos.x < r.xMax && pos.y >= r.yMin && pos.y < r.yMax) return positionToCoords(pos)
  }
  return null
}
