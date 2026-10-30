import { cmd } from '../cmd'
import { state, parentOf, markComponentDeleted } from '../state'
import { NODES_COMPONENT } from '../hierarchy-order'
import { NAME_COMPONENT } from '../custom-components'
import { compositeToSnapshotName, isAuthoredEntity, isSavableComponent } from '../composite'
import { sleep } from '../utils'
import { isObj, stripCase, writeComponent, writeDelete, applyExternalEntityDelete } from './mutate'
import {
  ensureProjectScenePinned,
  reloadAfter,
  reloadSnapshot,
  SETTLE_MS
} from './session'
import { allocateNamedEntities } from './entities'
import { deleteComponent } from './component-ops'


function isImportableEntity(eid: number): boolean {
  return Number.isFinite(eid) && eid !== 0 && isAuthoredEntity(eid)
}

function unwrapCompositeJson(raw: unknown): unknown {
  return isObj(raw) && 'json' in raw ? (raw as { json: unknown }).json : raw
}

function remapId(id: unknown, idMap: Map<number, number>): unknown {
  if (typeof id !== 'number') return id
  const mapped = idMap.get(id)
  return mapped === undefined ? id : mapped
}

function remapComponentRefs(
  snapshotName: string,
  value: unknown,
  idMap: Map<number, number>
): unknown {
  if (!isObj(value)) return value
  switch (snapshotName) {
    case 'Transform':
      if ('parent' in value) value.parent = remapId(value.parent, idMap)
      return value
    case 'asset-packs::Actions':
    case 'asset-packs::States':
    case 'asset-packs::Counter':
      if ('id' in value) value.id = remapId(value.id, idMap)
      return value
    case 'asset-packs::Triggers': {
      const triggers = (value as { value?: unknown }).value
      if (Array.isArray(triggers)) {
        for (const t of triggers) {
          if (!isObj(t)) continue
          if (Array.isArray(t.actions)) {
            for (const a of t.actions) if (isObj(a) && 'id' in a) a.id = remapId(a.id, idMap)
          }
          if (Array.isArray(t.conditions)) {
            for (const c of t.conditions) if (isObj(c) && 'id' in c) c.id = remapId(c.id, idMap)
          }
        }
      }
      return value
    }
    default:
      return value
  }
}

function readCompositeEntities(blocks: unknown[]): {
  comps: Map<number, Map<string, unknown>>
  names: Map<number, string>
} {
  const comps = new Map<number, Map<string, unknown>>()
  const names = new Map<number, string>()
  for (const block of blocks) {
    if (!isObj(block) || typeof block.name !== 'string' || !isObj(block.data)) continue
    if (block.name === NAME_COMPONENT) {
      for (const [idKey, raw] of Object.entries(block.data)) {
        const eid = Number(idKey)
        if (!isImportableEntity(eid)) continue
        const v = unwrapCompositeJson(raw)
        if (isObj(v) && typeof v.value === 'string') names.set(eid, v.value)
      }
      continue
    }
    const snapshotName = compositeToSnapshotName(block.name)
    if (snapshotName === undefined || snapshotName.startsWith('inspector::')) continue
    for (const [idKey, raw] of Object.entries(block.data)) {
      const eid = Number(idKey)
      if (!isImportableEntity(eid)) continue
      let entry = comps.get(eid)
      if (entry === undefined) {
        entry = new Map()
        comps.set(eid, entry)
      }
      entry.set(snapshotName, stripCase(unwrapCompositeJson(raw)))
    }
  }
  return { comps, names }
}

let importing = false

export async function importComposite(
  compositeText: string,
  opts?: { namedOnly?: boolean; preserveIds?: boolean }
): Promise<{ imported: number; idMap: Record<string, number> }> {
  if (importing) throw new Error('A scene import is already in progress')
  if (state.status !== 'ready') {
    throw new Error('The scene is not ready for importing')
  }
  importing = true
  try {
    let parsed: unknown
    try {
      parsed = JSON.parse(compositeText)
    } catch (e) {
      throw new Error(`The composite is not valid JSON: ${String(e)}`)
    }
    const blocks =
      isObj(parsed) && Array.isArray((parsed as { components?: unknown }).components)
        ? (parsed as { components: unknown[] }).components
        : []

    const { comps, names } = readCompositeEntities(blocks)

    const order = [...new Set([...comps.keys(), ...names.keys()])]
      .filter((eid) => !opts?.namedOnly || names.has(eid))
      .sort((a, b) => a - b)
    if (order.length === 0) return { imported: 0, idMap: {} }

    const projectHash = state.scene?.hash
    if (projectHash !== undefined) {
      const pinned = await ensureProjectScenePinned(projectHash)
      if (!pinned) {
        throw new Error('The project scene could not be selected for importing')
      }
    }

    const newIds = await allocateNamedEntities(
      order.map((eid) => ({ value: names.get(eid) ?? 'Entity' })),
      opts?.preserveIds ? order : undefined
    )
    if (newIds.length !== order.length || newIds.some((id) => id == null)) {
      throw new Error('Not all imported entities could be allocated')
    }
    const idMap = new Map<number, number>()
    for (let i = 0; i < order.length; i++) {
      const nid = newIds[i]
      if (nid !== null && nid !== undefined) idMap.set(order[i], nid)
    }

    const writes: Array<Promise<void>> = []
    for (const oldId of order) {
      const newId = idMap.get(oldId)
      const entry = comps.get(oldId)
      if (newId === undefined || entry === undefined) continue
      const eid = String(newId)
      for (const [name, value] of entry) {
        const remapped = remapComponentRefs(name, JSON.parse(JSON.stringify(value)), idMap)
        writes.push(writeComponent(eid, name, JSON.stringify(remapped)))
      }
    }
    await Promise.all(writes)

    if (!state.frozen && order.length > 0) {
      const last = idMap.get(order[order.length - 1])
      for (let attempt = 0; attempt < 6; attempt++) {
        await sleep(SETTLE_MS)
        await reloadSnapshot()
        if (last === undefined || state.snapshot[String(last)] !== undefined) break
      }
    }

    const idMapObj: Record<string, number> = {}
    for (const [o, n] of idMap) idMapObj[String(o)] = n
    console.log(`[import] hydrated ${idMap.size} authored entities from saved composite`)
    return { imported: idMap.size, idMap: idMapObj }
  } catch (e) {
    console.error('importComposite failed:', e)
    throw e
  } finally {
    importing = false
  }
}

export async function restoreComposite(compositeText: string): Promise<void> {
  let parsed: unknown
  try {
    parsed = JSON.parse(compositeText)
  } catch (e) {
    throw new Error(`restoreComposite: composite is not valid JSON: ${String(e)}`)
  }
  if (!isObj(parsed) || !Array.isArray(parsed.components)) throw new Error('restoreComposite: missing component list')
  const blocks = parsed.components as unknown[]
  const { comps, names } = readCompositeEntities(blocks)
  const snap = state.snapshot
  const failures: unknown[] = []
  const missing: number[] = []
  for (const eid of names.keys()) {
    const id = String(eid)
    if (snap[id] === undefined) {
      missing.push(eid)
      continue
    }
    const entry = comps.get(eid) ?? new Map<string, unknown>()
    entry.set(NAME_COMPONENT, { value: names.get(eid) })
    for (const name of Object.keys(snap[id])) {
      if (entry.has(name) || !isSavableComponent(name) || name.startsWith('inspector::')) continue
      try {
        await cmd.deleteComponent(id, name)
        delete snap[id][name]
        markComponentDeleted(id, name)
      } catch (e) {
        failures.push(e)
      }
    }
    for (const [n, v] of entry) {
      try {
        await writeComponent(id, n, JSON.stringify(v))
      } catch (e) {
        console.error('restoreComposite: overwrite failed', id, n, e)
        failures.push(e)
      }
    }
  }
  const extras = new Set(Object.keys(snap).filter(id =>
    isImportableEntity(Number(id)) && snap[id]?.[NAME_COMPONENT] !== undefined && !names.has(Number(id))
  ))
  const extraRoots = [...extras].filter(id => {
    let parent = parentOf(snap, id)
    const visited = new Set<string>([id])
    while (parent !== null && !visited.has(parent) && !extras.has(parent)) {
      visited.add(parent)
      parent = parentOf(snap, parent)
    }
    return parent === null || !extras.has(parent)
  })
  for (const id of extraRoots) {
      try {
        try {
          await writeDelete(id, true)
        } catch (error) {
          if (!new RegExp(`^(?:Error: )?entity ${id} does not exist$`).test(String(error))) throw error
          const current = await cmd.crdtSnapshot()
          if (Object.prototype.hasOwnProperty.call(current, id)) throw error
          applyExternalEntityDelete(id, true)
        }
      } catch (e) {
        console.error('restoreComposite: extra delete failed', id, e)
        failures.push(e)
      }
  }
  if (missing.length > 0) {
    const subset = {
      ...(parsed as Record<string, unknown>),
      components: blocks.map((b) => {
        if (!isObj(b) || !isObj((b as { data?: unknown }).data)) return b
        const data: Record<string, unknown> = {}
        for (const [k, v] of Object.entries((b as { data: Record<string, unknown> }).data)) {
          if (missing.includes(Number(k))) data[k] = v
        }
        return { ...(b as Record<string, unknown>), data }
      })
    }
    const result = await importComposite(JSON.stringify(subset), { namedOnly: true, preserveIds: true })
    if (result.imported !== missing.length) failures.push(new Error('Not all scene entities could be restored'))
  }
  for (const name of ['inspector::SceneMetadata-v3', NODES_COMPONENT]) {
    const block = blocks.find(block => isObj(block) && block.name === name)
    try {
      if (isObj(block) && isObj(block.data) && block.data['0'] !== undefined) {
        await writeComponent('0', name, JSON.stringify(unwrapCompositeJson(block.data['0'])))
      } else if (name !== NODES_COMPONENT && state.snapshot['0']?.[name] !== undefined) {
        await deleteComponent('0', name)
      }
    } catch (error) {
      failures.push(error)
    }
  }
  await reloadAfter()
  if (failures.length) throw new Error(`Scene restore failed (${failures.length} operation(s))`)
  console.log(`[restore] overwrote ${names.size - missing.length} entities, reimported ${missing.length}`)
}
