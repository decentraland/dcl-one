import type { Snapshot } from './state'

export const NODES_COMPONENT = 'inspector::Nodes'
export type HierarchyNode = { entity: number; open?: boolean; children: number[] }

export function hierarchyNodes(snapshot: Snapshot): HierarchyNode[] {
  const ids = Object.keys(snapshot).map(Number).filter(id => Number.isSafeInteger(id) && id >= 512)
  const saved = (snapshot['0']?.[NODES_COMPONENT] as { value?: HierarchyNode[] } | undefined)?.value ?? []
  const children = new Map<number, number[]>()
  for (const id of ids) {
    const raw = (snapshot[String(id)].Transform as { parent?: number } | undefined)?.parent ?? 0
    const parent = ids.includes(raw) && raw !== id ? raw : 0
    children.set(parent, [...(children.get(parent) ?? []), id])
  }
  return [0, ...ids].map(entity => {
    const previous = saved.find(node => node.entity === entity)
    const current = children.get(entity) ?? []
    const ordered = [...new Set([...(previous?.children ?? []), ...current])].filter(id => current.includes(id))
    return { entity, ...(previous?.open !== undefined ? { open: previous.open } : {}), children: ordered }
  })
}

export function hierarchyRoots(snapshot: Snapshot, ids: string[]): string[] {
  const unique = [...new Set(ids)]
  for (const id of unique) {
    if (!Number.isSafeInteger(Number(id)) || Number(id) < 512 || !snapshot[id]) throw new Error('Select authored entities to edit')
  }
  return unique.filter(id => {
    const seen = new Set([id])
    let parent = String((snapshot[id]?.Transform as { parent?: number } | undefined)?.parent ?? 0)
    while (parent !== '0') {
      if (seen.has(parent)) throw new Error('The hierarchy contains a cycle')
      if (unique.includes(parent)) return false
      seen.add(parent)
      parent = String((snapshot[parent]?.Transform as { parent?: number } | undefined)?.parent ?? 0)
    }
    return true
  })
}

export function hierarchyDescendants(snapshot: Snapshot, roots: string[]): string[] {
  const result = new Set(roots)
  const queue = [...roots]
  while (queue.length) {
    const parent = queue.shift()!
    for (const [id, comps] of Object.entries(snapshot)) {
      if (String((comps.Transform as { parent?: number } | undefined)?.parent ?? 0) === parent && !result.has(id)) {
        result.add(id)
        queue.push(id)
      }
    }
  }
  return [...result]
}
