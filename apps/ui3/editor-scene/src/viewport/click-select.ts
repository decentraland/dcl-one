import {
  engine,
  Transform,
  Raycast,
  RaycastResult,
  RaycastQueryType,
  inputSystem,
  InputAction,
  PointerEventType,
  PrimaryPointerInfo,
  type Entity
} from '@dcl/sdk/ecs'
import { cmd } from '../cmd'
import { log } from '../log'
import { state, selectionClick, clearSelection, parentOf } from '../state'
import { NAME_COMPONENT } from '../custom-components'
import { PICK_LAYER, GLTF, MESH_RENDERER, MESH_COLLIDER, pickApplied, synthesized } from './pick-layer'

function colliderMeshFromRenderer(mesh: unknown): Record<string, unknown> | undefined {
  if (typeof mesh !== 'object' || mesh === null) return undefined
  const m = mesh as Record<string, unknown>
  if ('box' in m) return { box: {} }
  if ('sphere' in m) return { sphere: {} }
  if ('cylinder' in m) return { cylinder: { ...(m.cylinder as object) } }
  if ('plane' in m) return { plane: {} }
  if ('gltf' in m) return { gltf: { ...(m.gltf as object) } }
  return undefined
}

// A write the scene state does not show yet is retried after this long: a restore of the authored
// scene (or the project's own main.crdt) replaces the components the pick mask was written on.
const PICK_RETRY_MS = 1000
const lastPickWrite = new Map<string, number>()

function syncPickColliders(): void {
  const now = Date.now()
  const writePick = (id: string, name: string, value: object, label: string): void => {
    lastPickWrite.set(id, now)
    void cmd.setComponent(id, name, JSON.stringify(value)).catch((e) => log.debug(`pick collider (${label}) failed`, e))
  }
  for (const [id, comps] of Object.entries(state.snapshot)) {
    const gltf = comps[GLTF] as { visibleMeshesCollisionMask?: number } | undefined
    const renderer = comps[MESH_RENDERER] as { mesh?: unknown } | undefined
    const existing = comps[MESH_COLLIDER] as { collisionMask?: number; mesh?: unknown } | undefined
    const picked = gltf !== undefined
      ? ((gltf.visibleMeshesCollisionMask ?? 0) & PICK_LAYER) !== 0
      : renderer === undefined || ((existing?.collisionMask ?? 0) & PICK_LAYER) !== 0
    if (picked) {
      pickApplied.add(id)
      continue
    }
    if (pickApplied.has(id) && now - (lastPickWrite.get(id) ?? 0) < PICK_RETRY_MS) continue
    if (gltf !== undefined) {
      const vis = gltf.visibleMeshesCollisionMask ?? 0
      writePick(id, GLTF, { ...gltf, visibleMeshesCollisionMask: vis | PICK_LAYER }, 'gltf')
      pickApplied.add(id)
      continue
    }
    if (renderer === undefined) continue
    if (existing !== undefined) {
      writePick(id, MESH_COLLIDER, { ...existing, collisionMask: (existing.collisionMask ?? 0) | PICK_LAYER }, 'mesh')
    } else {
      const value: Record<string, unknown> = { collisionMask: PICK_LAYER }
      const mesh = colliderMeshFromRenderer(renderer.mesh)
      if (mesh !== undefined) value.mesh = mesh
      writePick(id, MESH_COLLIDER, value, 'synth')
      synthesized.add(id)
    }
    pickApplied.add(id)
  }
}

function namedAncestor(id: string): string | null {
  let cur: string | null = id
  while (cur !== null && cur !== '0') {
    if (state.snapshot[cur]?.[NAME_COMPONENT] !== undefined) return cur
    cur = parentOf(state.snapshot, cur)
  }
  return null
}

let picker: Entity | null = null
let rayTs = 0
let pending: { shift: boolean; ctrl: boolean } | null = null

export function pickAtPointer(add: boolean, toggle: boolean): void {
  if (picker === null || state.playing) return
  const dir = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.worldRayDirection
  const camT = Transform.getOrNull(engine.CameraEntity)
  if (dir === undefined || camT === null) return
  Transform.createOrReplace(picker, { position: { ...camT.position } })
  rayTs += 1
  Raycast.createOrReplace(picker, {
    timestamp: rayTs,
    maxDistance: 1000,
    queryType: RaycastQueryType.RQT_QUERY_ALL,
    continuous: false,
    collisionMask: PICK_LAYER,
    direction: { $case: 'globalDirection', globalDirection: { ...dir } }
  })
  pending = { shift: add, ctrl: toggle }
}

function handlePickResult(): void {
  if (pending === null || picker === null) return
  const result = RaycastResult.getOrNull(picker)
  if (result === null || result.timestamp !== rayTs) return
  const p = pending
  pending = null
  const ordered = [...result.hits]
    .filter((h) => h.entityId !== undefined)
    .sort((a, b) => a.length - b.length)
  let picked: string | null = null
  for (const h of ordered) {
    const hit = String(h.entityId)
    if (!(hit in state.snapshot) || Number(hit) < 512) continue
    const id = namedAncestor(hit)
    if (id !== null) {
      picked = id
      break
    }
  }
  if (picked === null) {
    if (!p.shift && !p.ctrl) clearSelection()
    return
  }
  // Blender: shift-click selects/activates, shift-click on the active one deselects it.
  selectionClick(picked, p.shift, p.ctrl || (p.shift && state.activeEntity === picked))
}

export function setupMeshSelect(): void {
  picker = engine.addEntity()
  Transform.create(picker)
  engine.addSystem(() => {
    if (state.playing) {
      pending = null
      return
    }
    if (state.status === 'ready') syncPickColliders()
    handlePickResult()
  })
}

let pickDownXY: { x: number; y: number } | null = null
export function startGizmoPick(): void {
  engine.addSystem(() => {
    if (state.status !== 'ready' || state.playing) {
      pickDownXY = null
      return
    }
    const mode = state.activeAction
    if (mode !== 'translate' && mode !== 'rotate' && mode !== 'scale') {
      pickDownXY = null
      return
    }
    if (state.gizmoDragging) pickDownXY = null

    const p = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenCoordinates

    if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
      pickDownXY = state.gizmoHover === null && p !== undefined ? { x: p.x, y: p.y } : null
      return
    }
    if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_UP)) {
      const down = pickDownXY
      pickDownXY = null
      if (down === null) return
      if (p !== undefined && (Math.abs(p.x - down.x) > 4 || Math.abs(p.y - down.y) > 4)) return
      const add = inputSystem.isPressed(InputAction.IA_MODIFIER)
      const toggle = inputSystem.isPressed(InputAction.IA_WALK)
      pickAtPointer(add, toggle)
    }
  })
}

export function startSelectionHighlight(): void {
  let lastSig = ''
  engine.addSystem(() => {
    if (state.status !== 'ready') return
    const ids = state.playing ? [] : [...state.selected].sort()
    const sig = ids.join(',')
    if (sig === lastSig) return
    lastSig = sig
    cmd.highlight(ids).catch((e) => log.debug('highlight failed', e))
  })
}
