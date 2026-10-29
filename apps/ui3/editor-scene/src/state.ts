import { type LiveSceneInfo } from './bevy-api/interface'

export type Snapshot = Record<string, Record<string, unknown>>

export type InspectorStatus =
  | 'logging-in'
  | 'no-scene'
  | 'loading-snapshot'
  | 'ready'
  | 'error'

export const state = {
  status: 'logging-in' as InspectorStatus,
  error: '',
  scene: undefined as LiveSceneInfo | undefined,
  snapshot: {} as Snapshot,
  activeAction: 'translate' as string,
  lastTool: 'translate' as 'translate' | 'rotate' | 'scale',
  showLinks: false,
  camMode: 'none' as 'none' | 'free' | 'target',
  frozen: false,
  playing: false,
  componentNames: [] as string[],
  schemas: new Map<string, unknown>(),
  schemaPending: new Set<string>(),
  selected: new Set<string>(),
  activeEntity: null as string | null,
  pivotEach: false,
  orientGlobal: false,
  selectBox: null as
    | { startX: number; startY: number; curX: number; curY: number; add: boolean; remove: boolean }
    | null,
  gizmoHover: null as string | null,
  gizmoDragging: false,

  editedComponents: new Set<string>(),
  deletedComponents: new Set<string>(),
  deletedEntities: new Set<string>(),
  editorValues: new Map<string, unknown>()
}

export function markEdited(entityId: string, name: string, value: unknown): void {
  const key = `${entityId}/${name}`
  state.editedComponents.add(key)
  state.deletedComponents.delete(key)
  state.editorValues.set(key, value)
}

export function markComponentDeleted(entityId: string, name: string): void {
  const key = `${entityId}/${name}`
  state.deletedComponents.add(key)
  state.editedComponents.delete(key)
  state.editorValues.delete(key)
}

export function markEntityDeleted(entityId: string): void {
  state.deletedEntities.add(entityId)
}

const TOOLS = ['translate', 'rotate', 'scale']

export function setActiveAction(action: string): void {
  if (action === 'select') {
    state.activeAction = state.activeAction === 'select' ? state.lastTool : 'select'
    return
  }
  if (TOOLS.includes(action)) state.lastTool = action as 'translate' | 'rotate' | 'scale'
  state.activeAction = action
}

let revealedFor: string | null = null
/** Selecting an entity while the Select tool is active hands over to the last
 *  transform tool, so the gizmo appears the moment something is selected
 *  (hierarchy row, focus, canvas pick alike). */
export function revealGizmoForSelection(): void {
  const active = state.activeEntity
  if (active === revealedFor) return
  revealedFor = active
  if (active !== null && state.activeAction === 'select') state.activeAction = state.lastTool
}

export function clearSelection(): void {
  state.selected = new Set()
  state.activeEntity = null
}

function setWith<T>(s: ReadonlySet<T>, fn: (next: Set<T>) => void): Set<T> {
  const next = new Set(s)
  fn(next)
  return next
}
function mapWith<K, V>(m: ReadonlyMap<K, V>, fn: (next: Map<K, V>) => void): Map<K, V> {
  const next = new Map(m)
  fn(next)
  return next
}

export function setSelected(ids: Iterable<string>): void {
  state.selected = new Set(ids)
}

export function selectionClick(id: string, additive: boolean, toggle: boolean): void {
  if (toggle && state.selected.has(id)) {
    state.selected = setWith(state.selected, (s) => s.delete(id))
    if (state.activeEntity === id) {
      let last: string | null = null
      for (const v of state.selected) last = v
      state.activeEntity = last
    }
    return
  }
  state.selected = setWith(state.selected, (s) => {
    if (!toggle && !additive) s.clear()
    s.add(id)
  })
  state.activeEntity = id
}

export function applyBoxSelection(ids: string[], add: boolean, remove: boolean): void {
  state.selected = setWith(state.selected, (s) => {
    if (remove) {
      for (const id of ids) s.delete(id)
    } else {
      if (!add) s.clear()
      for (const id of ids) s.add(id)
    }
  })
  if (!remove && ids.length > 0) state.activeEntity = ids[ids.length - 1]
  if (state.activeEntity === null || !state.selected.has(state.activeEntity)) {
    let last: string | null = null
    for (const v of state.selected) last = v
    state.activeEntity = last
  }
}

export function topLevelSelected(snapshot: Snapshot): string[] {
  const out: string[] = []
  for (const id of state.selected) {
    let p = parentOf(snapshot, id)
    let nested = false
    while (p !== null) {
      if (state.selected.has(p)) {
        nested = true
        break
      }
      p = parentOf(snapshot, p)
    }
    if (!nested) out.push(id)
  }
  return out
}

export function setSnapshotComponent(id: string, name: string, value: unknown): void {
  state.snapshot = { ...state.snapshot, [id]: { ...state.snapshot[id], [name]: value } }
}
export function deleteSnapshotComponent(id: string, name: string): void {
  const entry = state.snapshot[id]
  if (entry === undefined || !(name in entry)) return
  const { [name]: _drop, ...rest } = entry
  state.snapshot = { ...state.snapshot, [id]: rest }
}
export function deleteSnapshotEntity(id: string): void {
  if (!(id in state.snapshot)) return
  const { [id]: _drop, ...rest } = state.snapshot
  state.snapshot = rest
}

export function setSchema(name: string, schema: unknown): void {
  state.schemas = mapWith(state.schemas, (m) => m.set(name, schema))
}

export type Forest = {
  roots: string[]
  children: Map<string, string[]>
}

export function parentOf(snapshot: Snapshot, id: string): string | null {
  if (id === '0') return null
  const transform = snapshot[id]?.Transform as { parent?: number } | undefined
  const parentId = transform?.parent === undefined ? '0' : String(transform.parent)
  return parentId === id ? null : parentId
}

export function buildForest(snapshot: Snapshot): Forest {
  const ids = Object.keys(snapshot)
  const present = new Set(ids)
  const children = new Map<string, string[]>()
  const roots: string[] = []

  for (const id of ids) {
    const parent = parentOf(snapshot, id)
    if (parent !== null && present.has(parent)) {
      const siblings = children.get(parent) ?? []
      siblings.push(id)
      children.set(parent, siblings)
    } else {
      roots.push(id)
    }
  }

  const byId = (a: string, b: string): number => Number(a) - Number(b)
  roots.sort(byId)
  for (const siblings of children.values()) siblings.sort(byId)
  return { roots, children }
}
