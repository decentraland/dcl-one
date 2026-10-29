// Holo-grid geometry for the editor viewport, kept free of SDK imports so it
// can be tested in node.
//
// Every slab and line spans a RUN of adjacent parcels, not one parcel. The
// first cut spawned a slab plus 18 lines per parcel: a 64x64 world needed
// ~78k entities, overflowed the SDK's 65535 entity ids, and the editor scene
// died before it could take the camera -- leaving the viewport on the
// player's spawn view with Run still offered.
export const PARCEL = 16
export const CELL = 2
export const MAJOR_EVERY = 4
// Well inside the 65535 entity ids, leaving the gizmos and the rest of the
// editor chrome their room. A 64x64 world costs 1216 at full detail.
export const GRID_ENTITY_BUDGET = 4096

export type GridParcel = { x: number; y: number }
/** Scene-space corner of a PARCEL-deep slab that is `width` long along X. */
export type GridSlab = { x: number; z: number; width: number }
/** Scene-space start of a line that runs `length` along X or along Z. */
export type GridLine = { alongX: boolean; x: number; z: number; length: number; major: boolean }
export type GridLayout = {
  slabs: GridSlab[]
  lines: GridLine[]
  detail: 'full' | 'major' | 'slabs' | 'none'
}

type Run = { at: number; from: number; to: number }

/** Contiguous runs of parcels along one axis, for each value of the other. */
function runs(parcels: readonly GridParcel[], across: (p: GridParcel) => number, along: (p: GridParcel) => number): Run[] {
  const lanes = new Map<number, Set<number>>()
  for (const p of parcels) {
    const lane = lanes.get(across(p)) ?? new Set<number>()
    lane.add(along(p))
    lanes.set(across(p), lane)
  }
  const out: Run[] = []
  for (const [at, lane] of lanes) {
    const sorted = [...lane].sort((a, b) => a - b)
    let from = sorted[0]
    for (let i = 1; i <= sorted.length; i++) {
      if (i < sorted.length && sorted[i] === sorted[i - 1] + 1) continue
      out.push({ at, from, to: sorted[i - 1] })
      from = sorted[i]
    }
  }
  return out
}

/** Places the grid with the scene's min parcel at the origin, at the densest detail the budget allows. */
export function gridLayout(parcels: readonly GridParcel[], budget = GRID_ENTITY_BUDGET): GridLayout {
  if (parcels.length === 0) return { slabs: [], lines: [], detail: 'none' }
  let minX = Infinity
  let minY = Infinity
  for (const p of parcels) {
    if (p.x < minX) minX = p.x
    if (p.y < minY) minY = p.y
  }
  const rows = runs(parcels, (p) => p.y, (p) => p.x)
  const columns = runs(parcels, (p) => p.x, (p) => p.y)
  const slabs = rows.map((r) => ({ x: (r.from - minX) * PARCEL, z: (r.at - minY) * PARCEL, width: (r.to - r.from + 1) * PARCEL }))
  if (slabs.length > budget) return { slabs: [], lines: [], detail: 'none' }

  for (const detail of ['full', 'major'] as const) {
    const offsets: { at: number; major: boolean }[] = []
    for (let i = 0; i <= PARCEL / CELL; i++) {
      const major = i % MAJOR_EVERY === 0
      if (detail === 'full' || major) offsets.push({ at: i * CELL, major })
    }
    if (slabs.length + (rows.length + columns.length) * offsets.length > budget) continue
    const lines: GridLine[] = []
    for (const r of rows) {
      for (const o of offsets) {
        lines.push({ alongX: true, x: (r.from - minX) * PARCEL, z: (r.at - minY) * PARCEL + o.at, length: (r.to - r.from + 1) * PARCEL, major: o.major })
      }
    }
    for (const c of columns) {
      for (const o of offsets) {
        lines.push({ alongX: false, x: (c.at - minX) * PARCEL + o.at, z: (c.from - minY) * PARCEL, length: (c.to - c.from + 1) * PARCEL, major: o.major })
      }
    }
    return { slabs, lines, detail }
  }
  return { slabs, lines: [], detail: 'slabs' }
}
