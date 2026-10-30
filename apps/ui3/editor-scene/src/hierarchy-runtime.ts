import { createComponentBatch } from './component-batch'
import { createMaterialBatch } from './material-batch'
import { cmd } from './cmd'
import { runtimeStatus } from './runtime-reset'
import { state, setSelected } from './state'
import { localRelativeTo } from './world-pos'
import { NAME_COMPONENT } from './custom-components'
import { hierarchyDescendants, hierarchyNodes, hierarchyRoots, NODES_COMPONENT } from './hierarchy-order'
import { componentClipboard, createHierarchyOperations, pastedComponent } from './hierarchy-operations'
import { writeComponent, writeDelete } from './inspector/mutate'
import { addComponent as addDefaultComponent, deleteComponent } from './inspector/component-ops'
import { ensureProjectScenePinned, reloadAfter } from './inspector/session'
import { exportComposite } from './inspector/save'
import { placeEntity } from './inspector/entities'
import { importComposite } from './inspector/hydrate'

async function ready() {
  if (state.status !== 'ready' || state.playing) throw new Error('Stop the preview before editing the hierarchy')
  if (!state.scene || !await ensureProjectScenePinned(state.scene.hash)) throw new Error('The project scene could not be selected')
}

export const moveHierarchy = createHierarchyOperations({
  snapshot: () => state.snapshot,
  ready,
  localRelativeTo,
  write: (entity, name, value) => writeComponent(entity, name, JSON.stringify(value)),
  refresh: () => reloadAfter()
})

export async function copyEntities(ids: unknown) {
  await ready()
  if (!Array.isArray(ids) || !ids.every(id => typeof id === 'string')) throw new Error('Select entities to copy')
  const roots = hierarchyRoots(state.snapshot, ids)
  if (!roots.length) throw new Error('Select entities to copy')
  const included = new Set(hierarchyDescendants(state.snapshot, roots))
  const definition = JSON.parse(await exportComposite()) as { components: Array<{ name: string; data: Record<string, unknown> }> }
  definition.components = definition.components.map(block => block.name === NODES_COMPONENT ? { ...block, data: { "0": { json: { value: [{ entity: 0, children: roots.map(Number) }, ...hierarchyNodes(state.snapshot).filter(node => included.has(String(node.entity)))] } } } } : ({
    ...block,
    data: Object.fromEntries(Object.entries(block.data).filter(([id]) => included.has(id)).map(([id, raw]) => {
      if (block.name !== 'core::Transform' || !roots.includes(id)) return [id, raw]
      const transform = { ...localRelativeTo(state.snapshot, id, '0'), parent: 0 }
      const value = raw as Record<string, unknown>
      return [id, value && typeof value === 'object' && 'json' in value ? { ...value, json: transform } : transform]
    }))
  })).filter(block => Object.keys(block.data).length)
  return { version: 1, roots, nodes: hierarchyNodes(state.snapshot).filter(node => included.has(String(node.entity))), composite: JSON.stringify(definition) }
}

export async function pasteEntities(payload: unknown, parent: unknown = '0') {
  await ready()
  if (!payload || typeof payload !== 'object' || typeof parent !== 'string') throw new Error('Copy entities before pasting')
  const source = payload as { version?: number; roots?: unknown; nodes?: unknown; composite?: unknown }
  if (source.version !== 1 || !Array.isArray(source.roots) || !source.roots.length || !source.roots.every(id => typeof id === 'string') || typeof source.composite !== 'string') throw new Error('Clipboard does not contain entities')
  if (parent !== '0' && (!state.snapshot[parent] || Number(parent) < 512)) throw new Error('The destination entity is unavailable')
  const savedNodes = source.nodes
  if (savedNodes !== undefined && (!Array.isArray(savedNodes) || !savedNodes.every(node => node && Number.isSafeInteger(node.entity) && Array.isArray(node.children) && node.children.every(Number.isSafeInteger)))) throw new Error('Clipboard contains an invalid hierarchy')
  const result = await importComposite(source.composite)
  const roots = source.roots.map(id => result.idMap[id as string]).filter(id => id !== undefined).map(String)
  if (roots.length !== source.roots.length) throw new Error('The copied entities could not be imported')
  if (parent !== '0') await moveHierarchy(roots, parent, 'inside')
  if (Array.isArray(savedNodes)) {
    const nodes = hierarchyNodes(state.snapshot)
    for (const saved of savedNodes as Array<{ entity: number; children: number[] }>) {
      const node = nodes.find(item => item.entity === result.idMap[String(saved.entity)])
      if (!node) continue
      const children = saved.children.map(id => result.idMap[String(id)]).filter((id): id is number => id !== undefined && node.children.includes(id))
      node.children = [...new Set([...children, ...node.children])]
    }
    await writeComponent('0', NODES_COMPONENT, JSON.stringify({ value: nodes }))
  }
  setSelected(roots)
  state.activeEntity = roots[0] ?? null
  await reloadAfter()
  return roots
}

export async function copyComponent(entity: unknown, name: unknown) {
  await ready()
  if (typeof entity !== 'string' || typeof name !== 'string') throw new Error('Select a component to copy')
  return componentClipboard(name, state.snapshot[entity]?.[name])
}

export async function pasteComponent(ids: unknown, name: unknown, payload: unknown) {
  await ready()
  if (!Array.isArray(ids) || !ids.length || !ids.every(id => typeof id === 'string' && state.snapshot[id]) || typeof name !== 'string' || name === NODES_COMPONENT || name === NAME_COMPONENT) throw new Error('Select entities and a component to paste')
  const writes = ids.map(id => ({ id, value: pastedComponent(name, payload, state.snapshot[id][name]) }))
  for (const { id, value } of writes) await writeComponent(id, name, JSON.stringify(value))
  await reloadAfter()
  return ids
}

export function orderHierarchyEntities<T extends { id: string }>(entities: T[]): T[] {
  const order = hierarchyNodes(state.snapshot).flatMap(node => node.children.map(String))
  return entities.sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id))
}

const componentWrites = {
  ready,
  snapshot: () => state.snapshot,
  write: (entity: string, name: string, value: unknown) => writeComponent(entity, name, JSON.stringify(value)),
  remove: deleteComponent,
  refresh: () => reloadAfter()
}

export const writeComponents = createComponentBatch(componentWrites)
export const writeMaterials = createMaterialBatch({
  ...componentWrites,
  context: async () => {
    if (state.playing || !state.scene) throw new Error('The project is no longer ready for authoring')
    const current = runtimeStatus(await cmd.sceneStats())
    if (current.hash !== state.scene.hash || current.broken) throw new Error('The project runtime changed during the material edit')
    return `${current.hash}/${current.instance}`
  },
  refresh: async () => {
    await reloadAfter()
    if (state.status !== 'ready') throw new Error(state.error || 'The edited materials could not be refreshed')
  }
})

export async function addEntity(name: unknown, parent: unknown, components: unknown, drop: unknown) {
  await ready()
  if (typeof name !== 'string' || typeof parent !== 'number' || !Number.isSafeInteger(parent) || (parent !== 0 && (!state.snapshot[String(parent)] || parent < 512))) throw new Error('Invalid entity destination')
  if (components !== null && components !== undefined && (typeof components !== 'object' || Array.isArray(components))) throw new Error('Invalid entity components')
  let point: { x: number; y: number } | null = null
  if (drop !== null && drop !== undefined) {
    const candidate = drop as { x?: unknown; y?: unknown }
    if (typeof candidate.x !== 'number' || typeof candidate.y !== 'number' || !Number.isFinite(candidate.x) || !Number.isFinite(candidate.y) || candidate.x < 0 || candidate.x > 1 || candidate.y < 0 || candidate.y > 1) throw new Error('Invalid drop position')
    point = { x: candidate.x, y: candidate.y }
  }
  return placeEntity(name, parent, components as Record<string, unknown> | null, point)
}

export async function addComponent(entity: unknown, name: unknown) {
  await ready()
  if (typeof entity !== 'string' || !/^\d+$/.test(entity) || !Number.isSafeInteger(Number(entity)) || Number(entity) < 512 || !state.snapshot[entity] || typeof name !== 'string' || !name.trim() || name === NAME_COMPONENT || name === NODES_COMPONENT) throw new Error('Select an authored entity and component to add')
  await addDefaultComponent(entity, name)
  return name
}

export async function removeComponent(entity: unknown, name: unknown) {
  await ready()
  if (typeof entity !== 'string' || Number(entity) < 512 || !state.snapshot[entity] || typeof name !== 'string' || name === NAME_COMPONENT || name === NODES_COMPONENT) throw new Error('Select an authored entity and component to remove')
  await deleteComponent(entity, name)
  await reloadAfter()
}

export async function removeEntities(ids: unknown) {
  await ready()
  if (!Array.isArray(ids) || !ids.length || !ids.every(id => typeof id === 'string')) throw new Error('Select authored entities to remove')
  const roots = hierarchyRoots(state.snapshot, ids)
  for (const id of roots) await writeDelete(id, true)
  setSelected([])
  state.activeEntity = null
  await reloadAfter()
  return roots
}
