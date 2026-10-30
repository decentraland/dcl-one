
import { state, type Snapshot } from './state'
import { isAuthoredEntity, isSavableComponent } from './composite'

export type DiffSource = 'initial' | 'editor' | 'live'

export type Cell = { present: boolean; value?: unknown }

export type DiffRow = {
  entityId: string
  component: string
  cells: Record<DiffSource, Cell>
  options: DiffSource[]
}

const ABSENT: Cell = { present: false }
const cell = (v: unknown): Cell => (v === undefined ? ABSENT : { present: true, value: v })

export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (typeof a === 'number' && typeof b === 'number') return Math.fround(a) === Math.fround(b)
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') return false
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false
    return a.every((v, i) => deepEqual(v, b[i]))
  }
  const ao = a as Record<string, unknown>
  const bo = b as Record<string, unknown>
  const keys = Object.keys(ao)
  if (keys.length !== Object.keys(bo).length) return false
  return keys.every((k) => k in bo && deepEqual(ao[k], bo[k]))
}

function cellsEqual(a: Cell, b: Cell): boolean {
  if (a.present !== b.present) return false
  return !a.present || deepEqual(a.value, b.value)
}

function editorCell(entityId: string, name: string, initial: Cell): Cell {
  const key = `${entityId}/${name}`
  if (state.deletedEntities.has(entityId) || state.deletedComponents.has(key)) return ABSENT
  return state.editorValues.has(key) ? cell(state.editorValues.get(key)) : initial
}

function distinctOptions(cells: Record<DiffSource, Cell>): DiffSource[] {
  const out: DiffSource[] = ['initial']
  if (!cellsEqual(cells.editor, cells.initial)) out.push('editor')
  if (!cellsEqual(cells.live, cells.initial) && !cellsEqual(cells.live, cells.editor)) out.push('live')
  return out
}

export function optionForSource(row: DiffRow, source: DiffSource): DiffSource {
  return row.options.find((o) => cellsEqual(row.cells[o], row.cells[source])) ?? row.options[0]
}

export function defaultSelection(row: DiffRow): DiffSource {
  return optionForSource(row, 'editor')
}

function splitKey(key: string): [string, string] {
  const i = key.indexOf('/')
  return [key.slice(0, i), key.slice(i + 1)]
}

export function computeSaveDiff(initial: Snapshot, live: Snapshot): DiffRow[] {
  const entityIds = new Set<string>(Object.keys(initial))
  for (const key of state.editedComponents) entityIds.add(splitKey(key)[0])
  for (const key of state.deletedComponents) entityIds.add(splitKey(key)[0])
  for (const eid of state.deletedEntities) entityIds.add(eid)

  const rows: DiffRow[] = []
  for (const entityId of entityIds) {
    if (!isAuthoredEntity(Number(entityId))) continue

    const names = new Set<string>()
    for (const n of Object.keys(initial[entityId] ?? {})) names.add(n)
    for (const n of Object.keys(live[entityId] ?? {})) names.add(n)
    for (const key of state.editedComponents) {
      const [e, n] = splitKey(key)
      if (e === entityId) names.add(n)
    }
    for (const key of state.deletedComponents) {
      const [e, n] = splitKey(key)
      if (e === entityId) names.add(n)
    }

    for (const name of names) {
      if (!isSavableComponent(name)) continue
      const initialC = cell(initial[entityId]?.[name])
      const cells: Record<DiffSource, Cell> = {
        initial: initialC,
        editor: editorCell(entityId, name, initialC),
        live: cell(live[entityId]?.[name])
      }
      if (cellsEqual(cells.initial, cells.editor) && cellsEqual(cells.editor, cells.live)) continue
      rows.push({ entityId, component: name, cells, options: distinctOptions(cells) })
    }
  }

  rows.sort(
    (a, b) =>
      Number(a.entityId) - Number(b.entityId) || a.component.localeCompare(b.component)
  )
  return rows
}

type AuthoredData = Record<string, Record<string, unknown>>

export function buildAuthoredFromSelection(
  initial: Snapshot,
  rows: DiffRow[],
  selection: Map<string, DiffSource>
): AuthoredData {
  const authored: AuthoredData = {}
  for (const [eid, comps] of Object.entries(initial)) {
    if (!isAuthoredEntity(Number(eid))) continue
    for (const [name, value] of Object.entries(comps)) {
      if (!isSavableComponent(name)) continue
      const entry = authored[eid] ?? (authored[eid] = {})
      entry[name] = value
    }
  }

  for (const row of rows) {
    const src = selection.get(`${row.entityId}/${row.component}`) ?? defaultSelection(row)
    const chosen = row.cells[src]
    const entry = authored[row.entityId] ?? (authored[row.entityId] = {})
    if (chosen.present) entry[row.component] = chosen.value
    else delete entry[row.component]
  }

  for (const eid of Object.keys(authored)) {
    if (Object.keys(authored[eid]).length === 0) delete authored[eid]
  }
  return authored
}
