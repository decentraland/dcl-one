import assert from 'node:assert/strict'
import { test } from 'node:test'
import { GRID_ENTITY_BUDGET, gridLayout, type GridParcel } from '../src/viewport/grid-layout.ts'

const square = (size: number): GridParcel[] =>
  Array.from({ length: size * size }, (_, i) => ({ x: i % size, y: Math.floor(i / size) }))
const entities = (layout: ReturnType<typeof gridLayout>) => layout.slabs.length + layout.lines.length

test('a single parcel keeps the per-parcel grid: one slab and nine lines each way', () => {
  const layout = gridLayout([{ x: 0, y: 0 }])
  assert.equal(layout.detail, 'full')
  assert.deepEqual(layout.slabs, [{ x: 0, z: 0, width: 16 }])
  assert.deepEqual(layout.lines.filter((l) => l.alongX).map((l) => l.z), [0, 2, 4, 6, 8, 10, 12, 14, 16])
  assert.deepEqual(layout.lines.filter((l) => !l.alongX).map((l) => l.x), [0, 2, 4, 6, 8, 10, 12, 14, 16])
  assert.ok(layout.lines.every((l) => l.length === 16))
  assert.deepEqual(layout.lines.filter((l) => l.major).map((l) => (l.alongX ? l.z : l.x)), [0, 8, 16, 0, 8, 16])
})

test('a 64x64 world fits the budget at full detail instead of overflowing the entity ids', () => {
  const layout = gridLayout(square(64))
  assert.equal(layout.detail, 'full')
  assert.equal(layout.slabs.length, 64)
  assert.ok(layout.slabs.every((s) => s.x === 0 && s.width === 1024))
  assert.equal(entities(layout), 64 + 64 * 9 * 2)
  assert.ok(entities(layout) <= GRID_ENTITY_BUDGET)
})

test('runs break at gaps and the min parcel sits at the origin', () => {
  const layout = gridLayout([{ x: -5, y: 10 }, { x: -4, y: 10 }, { x: -2, y: 10 }, { x: -2, y: 11 }])
  assert.deepEqual(layout.slabs, [
    { x: 0, z: 0, width: 32 },
    { x: 48, z: 0, width: 16 },
    { x: 48, z: 16, width: 16 }
  ])
  const column = layout.lines.filter((l) => !l.alongX && l.x === 48)
  assert.deepEqual(column.map((l) => [l.z, l.length]), [[0, 32]])
})

test('duplicate parcels do not duplicate geometry', () => {
  assert.deepEqual(gridLayout([{ x: 1, y: 1 }, { x: 1, y: 1 }]), gridLayout([{ x: 1, y: 1 }]))
})

test('over budget the grid drops minor lines, then all lines, then everything', () => {
  const parcels = square(4)
  assert.equal(gridLayout(parcels, 4 + 8 * 9).detail, 'full')
  const major = gridLayout(parcels, 4 + 8 * 9 - 1)
  assert.equal(major.detail, 'major')
  assert.ok(major.lines.every((l) => l.major))
  assert.equal(entities(major), 4 + 8 * 3)
  assert.deepEqual(gridLayout(parcels, 4 + 8 * 3 - 1), { slabs: gridLayout(parcels).slabs, lines: [], detail: 'slabs' })
  assert.deepEqual(gridLayout(parcels, 3), { slabs: [], lines: [], detail: 'none' })
  assert.deepEqual(gridLayout([]), { slabs: [], lines: [], detail: 'none' })
})
