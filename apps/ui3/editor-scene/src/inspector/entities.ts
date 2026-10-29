import { cmd } from '../cmd'
import { state, markEdited, setSelected } from '../state'
import {
  customComponentId,
  recordCustomTimestamp,
  encodeCustomComponent,
  NAME_COMPONENT
} from '../custom-components'
import { sleep } from '../utils'
import {
  applyLocalComponent,
  mergeKeepingOrder,
  writeComponent,
  writeDelete
} from './mutate'
import { ensureProjectScenePinned, reloadSnapshot, SETTLE_MS } from './session'
import { groundPointFromScreen } from '../camera/camera-projection'

const PARCEL_SIZE = 16
// Keep drops a hair inside the border so a 1m-wide model does not straddle it.
const PARCEL_MARGIN = 0.5

/** Clamp a ground point into the union of the scene's parcels (world space,
    min parcel at origin -- same mapping the ambiance grid uses). A drop past
    the border snaps to the nearest in-bounds point of the nearest parcel. */
function clampToParcels(x: number, z: number): { x: number; z: number } {
  const parcels = state.scene?.parcels ?? [{ x: 0, y: 0 }]
  let minX = Infinity
  let minY = Infinity
  for (const p of parcels) {
    if (p.x < minX) minX = p.x
    if (p.y < minY) minY = p.y
  }
  let best: { x: number; z: number } | null = null
  let bestD = Infinity
  for (const p of parcels) {
    const ox = (p.x - minX) * PARCEL_SIZE
    const oz = (p.y - minY) * PARCEL_SIZE
    const cx = Math.min(Math.max(x, ox + PARCEL_MARGIN), ox + PARCEL_SIZE - PARCEL_MARGIN)
    const cz = Math.min(Math.max(z, oz + PARCEL_MARGIN), oz + PARCEL_SIZE - PARCEL_MARGIN)
    const d = (cx - x) * (cx - x) + (cz - z) * (cz - z)
    if (d < bestD) {
      bestD = d
      best = { x: cx, z: cz }
    }
  }
  return best ?? { x, z }
}


async function newEntityIds(
  componentId: number,
  base64: string,
  count: number,
  ids?: number[]
): Promise<number[]> {
  return await cmd.newEntity(componentId, base64, count, ids)
}

export async function allocateNamedEntities(
  names: Array<{ value: string }>,
  ids?: number[]
): Promise<Array<number | null>> {
  const nameId = customComponentId(NAME_COMPONENT)
  const out: Array<number | null> = []
  for (const [index, name] of names.entries()) {
    const nameBytes =
      nameId !== undefined ? encodeCustomComponent(NAME_COMPONENT, name) : undefined
    if (nameId === undefined || nameBytes === undefined) {
      console.error('allocateNamedEntities: cannot encode Name to instantiate entity')
      out.push(null)
      continue
    }
    const [id] = await newEntityIds(nameId, nameBytes, 1, ids ? [ids[index]] : undefined)
    if (id === undefined) {
      out.push(null)
      continue
    }
    const eid = String(id)
    recordCustomTimestamp(eid, NAME_COMPONENT, 1)
    applyLocalComponent(eid, NAME_COMPONENT, JSON.stringify(name))
    markEdited(eid, NAME_COMPONENT, JSON.parse(JSON.stringify(name)))
    out.push(id)
  }
  return out
}

export async function createEntities(
  specs: Array<Record<string, unknown>>
): Promise<number[]> {
  if (specs.length === 0) return []
  const ids: number[] = []
  try {
    for (const components of specs) {
      const name = (components[NAME_COMPONENT] ?? { value: 'Entity' }) as { value: string }
      const [id] = await allocateNamedEntities([name])
      if (id === null || id === undefined) throw new Error('The scene did not allocate an entity')
      ids.push(id)
      const eid = String(id)

      for (const [n, value] of Object.entries(components)) {
        if (n === NAME_COMPONENT) continue
        await writeComponent(eid, n, JSON.stringify(value))
      }
    }

    if (!state.frozen) {
      const last = ids.length > 0 ? String(ids[ids.length - 1]) : null
      for (let attempt = 0; attempt < 6; attempt++) {
        await sleep(SETTLE_MS)
        await reloadSnapshot()
        if (last === null || state.snapshot[last] !== undefined) break
      }
    }
  } catch (e) {
    for (const id of ids) {
      try { await writeDelete(String(id), false) } catch (rollbackError) {
        throw new Error(`Entity creation failed and the new entity could not be removed: ${String(rollbackError)}; ${String(e)}`)
      }
    }
    throw e
  }
  return ids
}

export async function placeEntity(
  name: string,
  parent: number,
  components?: Record<string, unknown> | null,
  dropPoint?: { x: number; y: number } | null
): Promise<string> {
  // Default landing spot is the parcel center; a drag-drop carries the
  // normalized viewport point, unprojected onto the ground plane and clamped
  // into the scene's parcel bounds -- the E2E proof landed a drop at x=-2.1,
  // outside the 16x16 parcel, where nothing is editable or publishable.
  let position: { x: number; y: number; z: number } = { x: 8, y: 1, z: 8 }
  if (dropPoint != null) {
    const ground = groundPointFromScreen(dropPoint.x, dropPoint.y)
    if (ground !== null) {
      const clamped = clampToParcels(ground.x, ground.z)
      position = { x: clamped.x, y: 0, z: clamped.z }
    }
  }
  const spec: Record<string, unknown> = {
    Transform: {
      position,
      rotation: { x: 0, y: 0, z: 0, w: 1 },
      scale: { x: 1, y: 1, z: 1 },
      parent
    },
    [NAME_COMPONENT]: { value: name || 'Entity' }
  }
  if (components && typeof components === 'object') {
    for (const [k, v] of Object.entries(components)) spec[k] = mergeKeepingOrder(spec[k], v)
  }
  const projectHash = state.scene?.hash
  if (projectHash !== undefined) {
    const pinned = await ensureProjectScenePinned(projectHash)
    if (!pinned) {
      throw new Error('The project scene could not be selected for placement')
    }
  }
  const ids = await createEntities([spec])
  if (ids.length > 0) {
    const eid = String(ids[0])
    setSelected([eid])
    state.activeEntity = eid
    return eid
  }
  throw new Error('The scene did not create the item')
}
