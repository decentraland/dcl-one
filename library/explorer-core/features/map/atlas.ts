// Genesis City map tile sources and the coordinate math shared by the full-screen MapPage
// and the HUD minimap.
//
// Two independent tile sources, matching the two DOM minimap styles:
//   satellite — the 8×8 grid of pre-baked jpg chunks (same source/geometry unity-explorer's
//               SatelliteChunkController uses). Fixed extent: outside the grid there is no imagery.
//   parcel    — the coloured parcel atlas rendered on demand by api.decentraland.org, as one
//               image centred near the player.
//
// Coordinates: a parcel is 16 m. Parcel (x, y) maps to world (x*16, z=y*16); parcel y and
// world z increase in the same direction, while screen y grows downward — hence the flip in
// `atlasPx`. "Atlas px" is the untransformed tile-grid space, before any pan/zoom/rotation.

import type { SatelliteView } from '../../engine/generated/index'
import { serviceUrl } from '@hud-host'

export const PARCEL_METERS = 16

// ---- satellite atlas -------------------------------------------------------

export const TILE_BASE_URL = 'https://media.githubusercontent.com/media/genesis-city/parcels/new-client-images/maps/lod-0/3/'
// The satellite map Genesis City realms advertise (`configurations.map.satelliteView` in their
// `/about`). The tiles above are that map, so a realm is only treated as Genesis City — DOM map
// styles, place markers — when it advertises this map's url.
export const GENESIS_SATELLITE_VIEW: SatelliteView = {
  version: 'v1',
  baseUrl: 'https://genesis.city/map/latest',
  suffixUrl: '.jpg',
  topLeftOffset: { x: -2, y: -6 }
}

export function isGenesisSatelliteView(view: SatelliteView | null): boolean {
  return view !== null && view.baseUrl === GENESIS_SATELLITE_VIEW.baseUrl
}

export const GRID = 8 // 8×8 satellite chunks
export const PARCELS_PER_TILE = 40 // one chunk spans 40 parcels
export const SPAN = GRID * PARCELS_PER_TILE // 320 parcels across
// Unity places the top-left chunk's center at parcel (-133, 132); chunks are 40 wide, so the
// atlas's top-left corner sits at parcel (-153, 152). x increases right, y increases up.
export const ORIGIN_X = -153 // parcel x at the atlas left edge
export const ORIGIN_Y = 152 // parcel y at the atlas top edge
export const SIZE = 8 // px per parcel in the base (untransformed) atlas

/** File {col}%2C{row}.jpg is the chunk at column `col` (left→right), row `row` (top→bottom). */
export function tileUrl(col: number, row: number): string {
  return `${TILE_BASE_URL}${col}%2C${row}.jpg`
}

/** Position of a world-metres point in atlas px, at `size` px per parcel. */
export function atlasPx(worldX: number, worldZ: number, size: number): { x: number; y: number } {
  return {
    x: (worldX / PARCEL_METERS - ORIGIN_X) * size,
    y: (ORIGIN_Y - worldZ / PARCEL_METERS) * size
  }
}

export interface Chunk {
  col: number
  row: number
}

/**
 * The satellite chunks overlapping an axis-aligned window of atlas px, clamped to the grid.
 * The minimap uses this so it renders the handful of chunks it can actually show instead of
 * all 64 — the window must be the *rotated* extent (a circle's bounding box), not the
 * viewport rect, or chunks pop in at the corners while turning.
 */
export function chunksInRect(left: number, top: number, right: number, bottom: number, tilePx: number): Chunk[] {
  const lo = (v: number): number => Math.max(0, Math.min(GRID - 1, Math.floor(v / tilePx)))
  const c0 = lo(left)
  const c1 = lo(right)
  const r0 = lo(top)
  const r1 = lo(bottom)
  const out: Chunk[] = []
  for (let col = c0; col <= c1; col++) {
    for (let row = r0; row <= r1; row++) out.push({ col, row })
  }
  return out
}

// ---- parcel atlas ----------------------------------------------------------

// The API renders on demand, so we key each image to a 16-parcel chunk but request a wider
// 48-parcel span around it. That margin means walking within a chunk never refetches, and the
// image still covers the widest zoom (768 m = 48 parcels) without sampling past its edge.
const PARCEL_TILE_CHUNK = 16
const PARCEL_TILE_PARCELS = 48
const PARCEL_TILE_PX_PER_PARCEL = 16

export interface ParcelTile {
  url: string
  /** Centre of the image, in world metres. */
  centerX: number
  centerZ: number
  /** Extent the image covers, in metres (square). */
  meters: number
}

/** The parcel-atlas image covering a world position. Stable while the player stays in the
 *  same 16-parcel chunk, so it can be used directly as an `<img src>` without thrashing. */
export function parcelTileFor(worldX: number, worldZ: number): ParcelTile {
  const chunkX = Math.floor(worldX / PARCEL_METERS / PARCEL_TILE_CHUNK)
  const chunkY = Math.floor(worldZ / PARCEL_METERS / PARCEL_TILE_CHUNK)
  const centerParcelX = chunkX * PARCEL_TILE_CHUNK + PARCEL_TILE_CHUNK / 2
  const centerParcelY = chunkY * PARCEL_TILE_CHUNK + PARCEL_TILE_CHUNK / 2
  const px = PARCEL_TILE_PARCELS * PARCEL_TILE_PX_PER_PARCEL
  return {
    url: `${serviceUrl('mapApi')}/v1/map.png?center=${centerParcelX},${centerParcelY}&width=${px}&height=${px}&size=${PARCEL_TILE_PX_PER_PARCEL}`,
    centerX: centerParcelX * PARCEL_METERS,
    centerZ: centerParcelY * PARCEL_METERS,
    meters: PARCEL_TILE_PARCELS * PARCEL_METERS
  }
}

// ---- parcels map ----

// The full-screen map's Parcels type: the same on-demand images as above, laid out as a fixed
// grid of square chunks, each centred on a parcel and 51 parcels wide.
const PARCELS_CHUNK_PX = 1020
const PARCELS_CHUNK_PARCEL_PX = 20
/** Parcels across one chunk. */
export const PARCELS_CHUNK_PARCELS = PARCELS_CHUNK_PX / PARCELS_CHUNK_PARCEL_PX
const PARCELS_GRID_MIN = -150
const PARCELS_GRID_MAX = 175

export interface ParcelsChunk {
  /** The parcel the image is centred on. */
  x: number
  y: number
  url: string
}

/** Every chunk of the parcels map, west to east then south to north. */
export function parcelsChunks(): ParcelsChunk[] {
  const base = `${serviceUrl('mapApi')}/v1/map.png`
  const out: ParcelsChunk[] = []
  for (let x = PARCELS_GRID_MIN; x <= PARCELS_GRID_MAX; x += PARCELS_CHUNK_PARCELS) {
    for (let y = PARCELS_GRID_MIN; y <= PARCELS_GRID_MAX; y += PARCELS_CHUNK_PARCELS) {
      out.push({ x, y, url: `${base}?center=${x},${y}&width=${PARCELS_CHUNK_PX}&height=${PARCELS_CHUNK_PX}&size=${PARCELS_CHUNK_PARCEL_PX}` })
    }
  }
  return out
}

// ---- satellite chunks in map units ----

/** Centre of satellite chunk (col, row) in parcels: the grid starts three parcels west and two
 *  north of the city's corner, where the imagery's border lies outside the world. */
export function satelliteChunkCenter(col: number, row: number): { x: number; y: number } {
  return { x: -133 + col * PARCELS_PER_TILE, y: 132 - row * PARCELS_PER_TILE }
}

// ---- satellite detail pyramid ----------------------------------------------

// Past the base chunks (level 3 below) the same 8×8 grid has a pyramid of finer tiles: level L
// splits it into 2^L × 2^L tiles of 1024 px, `i` counting east and `j` south from the atlas's
// top-left corner. Level 3 is one base chunk; level 8 is 1.25 parcels per tile. The tiles are
// Basis KTX2 files the engine decodes, listed per section (one section per base chunk) in a
// manifest. Rects here are in parcel space — the atlas's own axes, x east and y north — and a
// parcel-space unit is exactly one map unit divided by `PARCEL_UNITS`.

export const DETAIL_BASE_LEVEL = 3
export const DETAIL_MAX_LEVEL = 8
export const DETAIL_TILE_PX = 1024
/** Screen px per parcel at which a level-3 tile shows one image px per screen px. */
export const BASE_PX_PER_PARCEL = DETAIL_TILE_PX / PARCELS_PER_TILE
/** The most tiles a view may ask for; a finer level than this allows is stepped down from. */
export const MAX_DETAIL_TILES = 64

export interface ParcelRect {
  xMin: number
  yMin: number
  xMax: number
  yMax: number
}

export interface TileId {
  level: number
  i: number
  j: number
}

export interface TileRange {
  level: number
  i0: number
  i1: number
  j0: number
  j1: number
}

/** The pyramid level whose tiles best match `screenPxPerParcel` device px per parcel, biased 0.4
 *  of a level toward sharper imagery. */
export function levelFor(screenPxPerParcel: number): number {
  if (!(screenPxPerParcel > 0)) return DETAIL_BASE_LEVEL
  const level = DETAIL_BASE_LEVEL + Math.round(Math.log2(screenPxPerParcel / BASE_PX_PER_PARCEL) + 0.4)
  return Math.min(DETAIL_MAX_LEVEL, Math.max(DETAIL_BASE_LEVEL, level))
}

/** Tiles along one side of the grid at `level`. */
export const tilesPerSide = (level: number): number => 2 ** level

/** Parcels along one tile's side at `level`. */
export const tileParcels = (level: number): number => SPAN / tilesPerSide(level)

/** Indices lo..hi of the tiles of side `size` that overlap [min, max] measured from the grid edge,
 *  clamped to `count` tiles; null when the span lies wholly outside the grid. */
function axisRange(min: number, max: number, size: number, count: number): [number, number] | null {
  const lo = Math.floor(min / size)
  const hi = Math.max(lo, Math.ceil(max / size) - 1)
  if (hi < 0 || lo > count - 1) return null
  return [Math.max(0, lo), Math.min(count - 1, hi)]
}

/** The tiles of `level` overlapping a parcel-space rect, clamped to the grid; null when the rect
 *  lies outside it. */
export function tileRange(rect: ParcelRect, level: number): TileRange | null {
  const size = tileParcels(level)
  const count = tilesPerSide(level)
  const cols = axisRange(rect.xMin - ORIGIN_X, rect.xMax - ORIGIN_X, size, count)
  const rows = axisRange(ORIGIN_Y - rect.yMax, ORIGIN_Y - rect.yMin, size, count)
  if (cols == null || rows == null) return null
  return { level, i0: cols[0], i1: cols[1], j0: rows[0], j1: rows[1] }
}

const rangeCount = (r: TileRange): number => (r.i1 - r.i0 + 1) * (r.j1 - r.j0 + 1)

/**
 * The detail tiles a view needs: the level `levelFor` picks, stepped down until the rect covers at
 * most `MAX_DETAIL_TILES`. Level 3 is the base chunks' own imagery, so it yields no tiles.
 */
export function planDetail(rect: ParcelRect, screenPxPerParcel: number): { level: number; tiles: TileId[] } {
  let level = levelFor(screenPxPerParcel)
  let range = level > DETAIL_BASE_LEVEL ? tileRange(rect, level) : null
  while (range != null && level > DETAIL_BASE_LEVEL && rangeCount(range) > MAX_DETAIL_TILES) {
    level--
    range = level > DETAIL_BASE_LEVEL ? tileRange(rect, level) : null
  }
  if (range == null || level <= DETAIL_BASE_LEVEL) return { level, tiles: [] }
  const tiles: TileId[] = []
  for (let j = range.j0; j <= range.j1; j++) {
    for (let i = range.i0; i <= range.i1; i++) tiles.push({ level, i, j })
  }
  return { level, tiles }
}

/** A tile's parcel-space rect. */
export function tileRect(tile: TileId): ParcelRect {
  const size = tileParcels(tile.level)
  const xMin = ORIGIN_X + tile.i * size
  const yMax = ORIGIN_Y - tile.j * size
  return { xMin, yMin: yMax - size, xMax: xMin + size, yMax }
}

/** The tile one level up that contains `tile`. */
export const tileParent = (tile: TileId): TileId => ({ level: tile.level - 1, i: tile.i >> 1, j: tile.j >> 1 })

export const tileKey = (tile: TileId): string => `${tile.level}/${tile.i},${tile.j}`

/** The manifest section (the base chunk's column and row) a tile belongs to. */
export function tileSection(tile: TileId): { sx: number; sy: number } {
  const shift = tile.level - DETAIL_BASE_LEVEL
  return { sx: tile.i >> shift, sy: tile.j >> shift }
}

/** A rect grown by `share` of its own size on every side. */
export function growRect(rect: ParcelRect, share: number): ParcelRect {
  const dx = (rect.xMax - rect.xMin) * share
  const dy = (rect.yMax - rect.yMin) * share
  return { xMin: rect.xMin - dx, yMin: rect.yMin - dy, xMax: rect.xMax + dx, yMax: rect.yMax + dy }
}

export function rectContains(outer: ParcelRect, inner: ParcelRect): boolean {
  return inner.xMin >= outer.xMin && inner.yMin >= outer.yMin && inner.xMax <= outer.xMax && inner.yMax <= outer.yMax
}

/** The detail tile versions, by section (`"sx,sy"`). A section it lacks has no tiles. */
export type SatelliteManifest = ReadonlyMap<string, string>

export const satelliteManifestUrl = (): string => `${serviceUrl('abgenCdn')}/SatelliteMaps/manifest.json`

/** The manifest's `sections` as a map, or null when the document is not one. */
export function parseSatelliteManifest(json: unknown): SatelliteManifest | null {
  if (typeof json !== 'object' || json === null) return null
  const sections = (json as { sections?: unknown }).sections
  if (typeof sections !== 'object' || sections === null) return null
  const out = new Map<string, string>()
  for (const [key, version] of Object.entries(sections)) {
    if (/^\d+,\d+$/.test(key) && typeof version === 'string' && version !== '') out.set(key, version)
  }
  return out
}

/** The tile's path under the tile source, or null when its section has no tiles. */
export function satelliteTilePath(manifest: SatelliteManifest, tile: TileId): string | null {
  const { sx, sy } = tileSection(tile)
  const version = manifest.get(`${sx},${sy}`)
  if (version === undefined) return null
  return `sections/${sx}%2C${sy}/${encodeURIComponent(version)}/${tile.level}/${tile.i}%2C${tile.j}.ktx2`
}
