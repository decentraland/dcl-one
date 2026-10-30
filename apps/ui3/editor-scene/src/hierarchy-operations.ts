import type { Snapshot } from './state'
import { hierarchyDescendants, hierarchyNodes, hierarchyRoots, NODES_COMPONENT } from './hierarchy-order'

type ComponentValue = Record<string, unknown>
export interface HierarchyDependencies {
  snapshot(): Snapshot
  ready(): Promise<void>
  localRelativeTo(snapshot: Snapshot, entity: string, parent: string): unknown
  write(entity: string, name: string, value: unknown): Promise<void>
  refresh(): Promise<void>
}

export function createHierarchyOperations(deps: HierarchyDependencies) {
  return async function moveHierarchy(ids: unknown, target: unknown, placement: unknown = 'inside') {
    if (!Array.isArray(ids) || !ids.every(id => typeof id === 'string') || typeof target !== 'string' || !['inside', 'before', 'after'].includes(String(placement))) {
      throw new Error('Invalid hierarchy move')
    }
    await deps.ready()
    const snapshot = deps.snapshot()
    const roots = hierarchyRoots(snapshot, ids)
    if (!roots.length) throw new Error('Select entities to move')
    if (target !== '0' && (!snapshot[target] || Number(target) < 512)) throw new Error('The destination entity is unavailable')
    if (target === '0' && placement !== 'inside') throw new Error('The scene root cannot be reordered')
    const parent = placement === 'inside' ? target : String((snapshot[target]?.Transform as { parent?: number } | undefined)?.parent ?? 0)
    const descendants = hierarchyDescendants(snapshot, roots)
    if (descendants.includes(target) || descendants.includes(parent)) throw new Error('An entity cannot be moved into itself or its children')
    const nodes = hierarchyNodes(snapshot)
    for (const node of nodes) node.children = node.children.filter(id => !roots.includes(String(id)))
    const destination = nodes.find(node => String(node.entity) === parent)
    if (!destination) throw new Error('The destination parent is unavailable')
    const index = placement === 'inside' ? destination.children.length : destination.children.indexOf(Number(target)) + (placement === 'after' ? 1 : 0)
    destination.children.splice(index, 0, ...roots.map(Number))
    const writes = roots.map(id => ({ id, value: { ...(deps.localRelativeTo(snapshot, id, parent) as ComponentValue), parent: Number(parent) } }))
    const previous = new Map(roots.map(id => [id, JSON.parse(JSON.stringify(snapshot[id].Transform ?? {})) as unknown]))
    const completed: string[] = []
    try {
      for (const { id, value } of writes) {
        await deps.write(id, 'Transform', value)
        completed.push(id)
      }
      await deps.write('0', NODES_COMPONENT, { value: nodes })
    } catch (error) {
      let rollbackFailed = false
      for (const id of completed.reverse()) {
        try { await deps.write(id, 'Transform', previous.get(id)) } catch { rollbackFailed = true }
      }
      if (rollbackFailed) throw new Error(`Some entities could not be restored after the move failed. Reconnect and check the hierarchy. ${String(error)}`)
      throw error
    }
    await deps.refresh()
    return roots
  }
}

export function componentClipboard(name: string, value: unknown) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Component values are unavailable')
  const copy = JSON.parse(JSON.stringify(value)) as ComponentValue
  if (name === 'Transform' || name === 'core::Transform' || name === 'UiTransform' || name === 'core::UiTransform') delete copy.parent
  if (name === 'UiTransform' || name === 'core::UiTransform') delete copy.rightOf
  return { __dclComponent: name.includes('::') ? name : `core::${name}`, value: copy }
}

export function pastedComponent(name: string, payload: unknown, current: unknown) {
  if (!payload || typeof payload !== 'object') throw new Error('Copy component values before pasting')
  const source = payload as { __dclComponent?: string; value?: unknown }
  const expected = name.includes('::') ? name : `core::${name}`
  if (source.__dclComponent !== expected) throw new Error(`Clipboard does not contain ${name} values`)
  const { value } = componentClipboard(name, source.value)
  return { ...(current && typeof current === 'object' ? current : {}), ...value }
}
