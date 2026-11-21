import { describe, it, expect } from 'vitest'
import {
  BASE_PX_PER_PARCEL,
  MAX_DETAIL_TILES,
  ORIGIN_X,
  ORIGIN_Y,
  PARCEL_METERS,
  SPAN,
  atlasPx,
  chunksInRect,
  growRect,
  levelFor,
  parcelTileFor,
  parseSatelliteManifest,
  planDetail,
  rectContains,
  satelliteTilePath,
  tileKey,
  tileParent,
  tileRange,
  tileRect,
  tileSection,
  tileUrl
} from '../features/map/atlas'

// One satellite chunk spans 40 parcels; at the minimap's 8 px per parcel that's 320 px.
const TILE_PX = 320

describe('atlasPx', () => {
  it('puts the atlas top-left corner at (0, 0) whatever the scale', () => {
    const corner = { x: ORIGIN_X * PARCEL_METERS, z: ORIGIN_Y * PARCEL_METERS }
    expect(atlasPx(corner.x, corner.z, 8)).toEqual({ x: 0, y: 0 })
    expect(atlasPx(corner.x, corner.z, 32)).toEqual({ x: 0, y: 0 })
  })

  it('flips z: parcel y grows north, screen y grows down', () => {
    const at0 = atlasPx(0, 0, 8)
    const oneParcelNorth = atlasPx(0, PARCEL_METERS, 8)
    expect(oneParcelNorth.y).toBe(at0.y - 8)
    expect(oneParcelNorth.x).toBe(at0.x)
  })

  it('does not flip x: world x and screen x grow together', () => {
    const at0 = atlasPx(0, 0, 8)
    expect(atlasPx(PARCEL_METERS, 0, 8).x).toBe(at0.x + 8)
  })

  it('scales linearly with px-per-parcel', () => {
    const small = atlasPx(1600, -800, 8)
    const big = atlasPx(1600, -800, 16)
    expect(big).toEqual({ x: small.x * 2, y: small.y * 2 })
  })
})

describe('chunksInRect', () => {
  it('returns the single chunk a small window sits inside', () => {
    expect(chunksInRect(10, 10, 300, 300, TILE_PX)).toEqual([{ col: 0, row: 0 }])
  })

  it('returns the 2×2 block when the window straddles a chunk boundary', () => {
    const chunks = chunksInRect(TILE_PX - 10, TILE_PX - 10, TILE_PX + 10, TILE_PX + 10, TILE_PX)
    expect(chunks).toHaveLength(4)
    expect(chunks).toEqual(
      expect.arrayContaining([
        { col: 0, row: 0 },
        { col: 0, row: 1 },
        { col: 1, row: 0 },
        { col: 1, row: 1 }
      ])
    )
  })

  it('covers the whole 8×8 grid for a window spanning the atlas', () => {
    expect(chunksInRect(0, 0, TILE_PX * 8 - 1, TILE_PX * 8 - 1, TILE_PX)).toHaveLength(64)
  })

  it('clamps rather than culls: a window off the atlas still yields the nearest edge chunk', () => {
    // Documents current behaviour — outside the grid there is no imagery, so the edge chunk is
    // rendered stretched instead of nothing at all.
    expect(chunksInRect(-5000, -5000, -4000, -4000, TILE_PX)).toEqual([{ col: 0, row: 0 }])
    expect(chunksInRect(99_000, 99_000, 100_000, 100_000, TILE_PX)).toEqual([{ col: 7, row: 7 }])
  })
})

describe('parcelTileFor', () => {
  it('centres the image on the 16-parcel chunk holding the position', () => {
    const tile = parcelTileFor(0, 0)
    // Chunk 0 spans parcels 0..15, so its centre parcel is (8, 8).
    expect(tile.centerX).toBe(8 * PARCEL_METERS)
    expect(tile.centerZ).toBe(8 * PARCEL_METERS)
    expect(tile.url).toContain('center=8,8')
  })

  it('is stable while the player stays in the same chunk', () => {
    const first = parcelTileFor(0, 0)
    const stillInside = parcelTileFor(15 * PARCEL_METERS, 15 * PARCEL_METERS)
    expect(stillInside.url).toBe(first.url)
    // One parcel further is the next chunk, and a different image.
    expect(parcelTileFor(16 * PARCEL_METERS, 0).url).not.toBe(first.url)
  })

  it('floors towards negative coordinates rather than towards zero', () => {
    const tile = parcelTileFor(-PARCEL_METERS, -PARCEL_METERS)
    expect(tile.url).toContain('center=-8,-8')
  })

  it('covers 48 parcels, wide enough for the widest zoom (768 m)', () => {
    expect(parcelTileFor(0, 0).meters).toBe(48 * PARCEL_METERS)
  })
})

describe('tileUrl', () => {
  it('encodes the comma between column and row', () => {
    expect(tileUrl(3, 5)).toMatch(/3%2C5\.jpg$/)
  })
})

describe('levelFor', () => {
  it('is the base level at the base density and below', () => {
    expect(levelFor(BASE_PX_PER_PARCEL)).toBe(3)
    expect(levelFor(1)).toBe(3)
    expect(levelFor(0)).toBe(3)
  })

  it('steps up one level per doubling, rounding 0.4 of a level toward sharper imagery', () => {
    expect(levelFor(BASE_PX_PER_PARCEL * 2)).toBe(4)
    expect(levelFor(BASE_PX_PER_PARCEL * 2 ** 2)).toBe(5)
    // a little over a tenth of a doubling is already enough for the next level
    expect(levelFor(BASE_PX_PER_PARCEL * 2 ** 0.15)).toBe(4)
    expect(levelFor(BASE_PX_PER_PARCEL * 2 ** 0.05)).toBe(3)
  })

  it('stops at level 8', () => {
    expect(levelFor(BASE_PX_PER_PARCEL * 32)).toBe(8)
    expect(levelFor(1e6)).toBe(8)
  })
})

describe('tileRange', () => {
  const whole = { xMin: ORIGIN_X, xMax: ORIGIN_X + SPAN, yMin: ORIGIN_Y - SPAN, yMax: ORIGIN_Y }

  it('covers the whole grid with every tile of the level', () => {
    expect(tileRange(whole, 3)).toEqual({ level: 3, i0: 0, i1: 7, j0: 0, j1: 7 })
    expect(tileRange(whole, 5)).toEqual({ level: 5, i0: 0, i1: 31, j0: 0, j1: 31 })
  })

  it('counts tiles east and south from the top-left corner', () => {
    // 10 parcels in from the corner, one parcel square: inside tile (0,0) of every level up to 5
    const r = { xMin: ORIGIN_X + 10, xMax: ORIGIN_X + 11, yMin: ORIGIN_Y - 11, yMax: ORIGIN_Y - 10 }
    expect(tileRange(r, 3)).toEqual({ level: 3, i0: 0, i1: 0, j0: 0, j1: 0 })
    // at level 4 a tile is 20 parcels, at level 8 1.25 → parcel 10 starts tile 8
    expect(tileRange(r, 8)).toEqual({ level: 8, i0: 8, i1: 8, j0: 8, j1: 8 })
  })

  it('does not take the neighbour of a rect that ends on a tile edge', () => {
    const r = { xMin: ORIGIN_X, xMax: ORIGIN_X + 40, yMin: ORIGIN_Y - 40, yMax: ORIGIN_Y }
    expect(tileRange(r, 3)).toEqual({ level: 3, i0: 0, i1: 0, j0: 0, j1: 0 })
  })

  it('clamps a rect that overhangs the grid', () => {
    const r = { xMin: ORIGIN_X - 100, xMax: ORIGIN_X + 50, yMin: ORIGIN_Y - 50, yMax: ORIGIN_Y + 100 }
    expect(tileRange(r, 3)).toEqual({ level: 3, i0: 0, i1: 1, j0: 0, j1: 1 })
  })

  it('is null for a rect wholly outside the grid', () => {
    expect(tileRange({ xMin: ORIGIN_X - 50, xMax: ORIGIN_X - 10, yMin: 0, yMax: 10 }, 4)).toBeNull()
    expect(tileRange({ xMin: 0, xMax: 10, yMin: ORIGIN_Y + 10, yMax: ORIGIN_Y + 50 }, 4)).toBeNull()
  })
})

describe('planDetail', () => {
  const BASE_PX_PARCEL_MAX = BASE_PX_PER_PARCEL * 32
  const around = (x: number, y: number, half: number) => ({ xMin: x - half, xMax: x + half, yMin: y - half, yMax: y + half })

  it('asks for no tiles at the base level', () => {
    expect(planDetail(around(0, 0, 20), BASE_PX_PER_PARCEL)).toEqual({ level: 3, tiles: [] })
  })

  it('lists the tiles of the chosen level', () => {
    const plan = planDetail(around(0, 0, 0.5), BASE_PX_PER_PARCEL * 32)
    expect(plan.level).toBe(8)
    expect(plan.tiles.length).toBeGreaterThan(0)
    expect(plan.tiles.every((t) => t.level === 8)).toBe(true)
  })

  it('steps the level down until at most 64 tiles are left', () => {
    // 40 parcels across: level 8 is 1024 tiles, 7 is 256, 6 is 81 (9 by 9), 5 is 25 (5 by 5)
    const plan = planDetail(around(0, 0, 20), BASE_PX_PARCEL_MAX)
    expect(plan.level).toBe(5)
    expect(plan.tiles).toHaveLength(25)
    expect(plan.tiles.length).toBeLessThanOrEqual(MAX_DETAIL_TILES)
  })

  it('falls back to the base imagery when even level 4 is too many tiles', () => {
    expect(planDetail(around(0, 0, 150), BASE_PX_PARCEL_MAX)).toEqual({ level: 3, tiles: [] })
  })

  it('has nothing to ask for off the grid', () => {
    expect(planDetail({ xMin: 500, xMax: 510, yMin: 0, yMax: 10 }, BASE_PX_PER_PARCEL * 32).tiles).toEqual([])
  })
})

describe('tile geometry', () => {
  it('puts tile (0,0) at the top-left corner', () => {
    expect(tileRect({ level: 3, i: 0, j: 0 })).toEqual({ xMin: ORIGIN_X, xMax: ORIGIN_X + 40, yMin: ORIGIN_Y - 40, yMax: ORIGIN_Y })
    expect(tileRect({ level: 4, i: 1, j: 1 })).toEqual({ xMin: ORIGIN_X + 20, xMax: ORIGIN_X + 40, yMin: ORIGIN_Y - 40, yMax: ORIGIN_Y - 20 })
  })

  it('has a parent that contains it', () => {
    const tile = { level: 8, i: 77, j: 130 }
    const parent = tileParent(tile)
    expect(parent).toEqual({ level: 7, i: 38, j: 65 })
    const a = tileRect(tile)
    const b = tileRect(parent)
    expect(a.xMin).toBeGreaterThanOrEqual(b.xMin)
    expect(a.xMax).toBeLessThanOrEqual(b.xMax)
    expect(a.yMin).toBeGreaterThanOrEqual(b.yMin)
    expect(a.yMax).toBeLessThanOrEqual(b.yMax)
  })

  it('names a tile by level and position', () => {
    expect(tileKey({ level: 6, i: 3, j: 9 })).toBe('6/3,9')
  })

  it('belongs to the base chunk that contains it', () => {
    expect(tileSection({ level: 3, i: 5, j: 2 })).toEqual({ sx: 5, sy: 2 })
    expect(tileSection({ level: 8, i: 255, j: 0 })).toEqual({ sx: 7, sy: 0 })
    expect(tileSection({ level: 5, i: 9, j: 13 })).toEqual({ sx: 2, sy: 3 })
  })
})

describe('rect helpers', () => {
  const r = { xMin: 0, yMin: 0, xMax: 10, yMax: 20 }

  it('grows a rect by a share of its size on every side', () => {
    expect(growRect(r, 0.5)).toEqual({ xMin: -5, yMin: -10, xMax: 15, yMax: 30 })
  })

  it('tells whether one rect holds another', () => {
    expect(rectContains(growRect(r, 0.25), r)).toBe(true)
    expect(rectContains(r, growRect(r, 0.25))).toBe(false)
    expect(rectContains(r, r)).toBe(true)
  })
})

describe('the tile manifest', () => {
  const manifest = parseSatelliteManifest({ sections: { '0,0': 'v 1', '3,4': '2026-11-19' } })

  it('keeps the sections with a version', () => {
    expect(manifest).toEqual(
      new Map([
        ['0,0', 'v 1'],
        ['3,4', '2026-11-19']
      ])
    )
  })

  it('drops malformed sections and rejects other documents', () => {
    expect(parseSatelliteManifest({ sections: { '0,0': 'v1', x: 'v1', '1,1': 3, '2,2': '' } })).toEqual(new Map([['0,0', 'v1']]))
    expect(parseSatelliteManifest(null)).toBeNull()
    expect(parseSatelliteManifest({})).toBeNull()
    expect(parseSatelliteManifest('sections')).toBeNull()
  })

  it('builds the tile path from its section, version, level and position', () => {
    expect(satelliteTilePath(manifest as ReadonlyMap<string, string>, { level: 3, i: 0, j: 0 })).toBe('sections/0%2C0/v%201/3/0%2C0.ktx2')
    // level 5 tile (13, 17) lies in section (3, 4)
    expect(satelliteTilePath(manifest as ReadonlyMap<string, string>, { level: 5, i: 13, j: 17 })).toBe('sections/3%2C4/2026-11-19/5/13%2C17.ktx2')
  })

  it('has no path for a section the manifest lacks', () => {
    expect(satelliteTilePath(manifest as ReadonlyMap<string, string>, { level: 4, i: 4, j: 0 })).toBeNull()
  })
})
