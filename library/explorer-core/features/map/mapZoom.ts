// The full-screen map's camera: seven zoom levels, the size rules that keep markers readable
// across them, and the bounds the camera may travel within. All lengths are map units
// (20 per parcel); "ortho" is half the height the view shows.

import { PARCEL_UNITS, VISIBLE_WORLD_BOUNDS, type Rect, type Vec2 } from './mapCoords'

/** Closest and farthest half-heights the map allows, in parcels. */
const ZOOM_RANGE_PARCELS = { near: 14, far: 157 }
const CLOSEST_ZOOM_PARCELS = 1

/** Markers are sized against this half-height: at it, a marker is one parcel wide. */
export const BASE_ZOOM = ZOOM_RANGE_PARCELS.near * PARCEL_UNITS
const FAR_ZOOM = ZOOM_RANGE_PARCELS.far * PARCEL_UNITS
const CLOSEST_ZOOM = CLOSEST_ZOOM_PARCELS * PARCEL_UNITS

/** How far each level sits between the farthest (0) and closest (1) view. */
const LEVEL_CURVE = [0, 0.14049491, 0.36240783, 0.5673127, 0.7490243, 0.9013244, 1]

export const ZOOM_LEVELS = LEVEL_CURVE.length
export const MAX_ZOOM_LEVEL = ZOOM_LEVELS - 1
export const DEFAULT_ZOOM_LEVEL = Math.floor((ZOOM_LEVELS - 1) / 2)
export const ZOOM_STEP_SECONDS = 0.3

export function clampZoomLevel(level: number): number {
  return Math.min(MAX_ZOOM_LEVEL, Math.max(0, level))
}

export function normalizedZoom(level: number): number {
  return LEVEL_CURVE[clampZoomLevel(level)]
}

export function orthoFor(normalized: number): number {
  const t = Math.min(1, Math.max(0, normalized))
  return FAR_ZOOM + (CLOSEST_ZOOM - FAR_ZOOM) * t
}

export function orthoForLevel(level: number): number {
  return orthoFor(normalizedZoom(level))
}

/** The scale that keeps a marker the same size on screen at every zoom. */
export function scaleForZoom(baseScale: number, ortho: number): number {
  return (ortho / BASE_ZOOM) * baseScale
}

const TITLE_Y_POSITION_OFFSET = -0.2

/** A marker's title against its marker's scale: above the base scale the title keeps its map
 *  size (so it shrinks on screen as the view pulls back); below, it shrinks with the marker. */
export function titleForScale(titleBaseY: number, titleBaseScale: number, baseScale: number, newScale: number): { y: number; scale: number } {
  const positionFactor = Math.max(newScale / baseScale, 1)
  const yOffset = (1 - positionFactor) * TITLE_Y_POSITION_OFFSET
  return {
    y: yOffset < 0.9 ? titleBaseY + yOffset : titleBaseY / positionFactor,
    scale: titleBaseScale / positionFactor
  }
}

/** Other players' dots keep their map size, and only hold their screen size closer than the base zoom. */
export function userMarkerScale(ortho: number): number {
  return PARCEL_UNITS * Math.min(ortho / BASE_ZOOM, 1)
}

const PIN_MIN_SCALE = 35
const PIN_SIZE_MULTIPLIER = 0.6

/** Scene pins: larger than place markers, and never smaller than their minimum once past the base zoom. */
export function pinMarkerScale(ortho: number): number {
  const ratio = ortho / BASE_ZOOM
  return ratio < 1 ? ratio * PIN_MIN_SCALE : Math.max(ratio * PIN_MIN_SCALE * PIN_SIZE_MULTIPLIER, PIN_MIN_SCALE)
}

const PATH_DOT_SIZE = 15

export function pathDotSize(ortho: number): number {
  return scaleForZoom(PATH_DOT_SIZE, ortho)
}

// The camera may travel past the world's edge by this share of its own half-size.
const EXTRA_MOVEMENT_PADDING = -0.3

function clamp(value: number, min: number, max: number): number {
  if (value < min) return min
  if (value > max) return max
  return value
}

/** Where the camera centre may sit for a view of half-height `ortho` and width/height `aspect`. */
export function cameraBounds(ortho: number, aspect: number): Rect {
  const world = VISIBLE_WORLD_BOUNDS
  const ySize = ortho
  const xSize = ortho * aspect
  const padX = xSize * EXTRA_MOVEMENT_PADDING
  const padY = ySize * EXTRA_MOVEMENT_PADDING

  let xMin = world.xMin + xSize + padX
  let xMax = world.xMax - xSize - padX
  let yMin = world.yMin + ySize + padY
  let yMax = world.yMax - ySize - padY

  if (world.xMax - world.xMin < 2 * xSize) {
    xMin = padX
    xMax = -padX
  }
  if (world.yMax - world.yMin < 2 * ySize) {
    yMin = padY
    yMax = -padY
  }
  return { xMin, yMin, xMax, yMax }
}

export function clampCamera(pos: Vec2, ortho: number, aspect: number): Vec2 {
  const b = cameraBounds(ortho, aspect)
  return { x: clamp(pos.x, b.xMin, b.xMax), y: clamp(pos.y, b.yMin, b.yMax) }
}

export interface MapView {
  /** Camera centre, map units. */
  x: number
  y: number
  /** Half the height shown, map units. */
  ortho: number
  /** Viewport size in layout px. */
  width: number
  height: number
}

export function pxPerUnit(view: MapView): number {
  return view.height / (2 * view.ortho)
}

export function mapToScreen(pos: Vec2, view: MapView): Vec2 {
  const k = pxPerUnit(view)
  return { x: view.width / 2 + (pos.x - view.x) * k, y: view.height / 2 - (pos.y - view.y) * k }
}

export function screenToMap(point: Vec2, view: MapView): Vec2 {
  const k = pxPerUnit(view)
  return { x: view.x + (point.x - view.width / 2) / k, y: view.y - (point.y - view.height / 2) / k }
}

/** Camera inertia after a drag: the share of velocity kept per second. */
export const DRAG_DECELERATION_RATE = 0.00001
export const CAMERA_MOVE_SECONDS = 1
export const CAMERA_CENTER_SECONDS = 0.5

export const easeOutQuart = (t: number): number => 1 - Math.pow(1 - t, 4)
