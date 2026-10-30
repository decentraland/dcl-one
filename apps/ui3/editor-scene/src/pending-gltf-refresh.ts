import type { Snapshot } from './state'

export interface PendingGltf {
  entity: string
  src: string
}

const gltfNames = ['GltfContainer', 'core::GltfContainer']
const loadingNames = ['GltfContainerLoadingState', 'core::GltfContainerLoadingState']

const object = (value: unknown): Record<string, unknown> | undefined =>
  value !== null && typeof value === 'object' ? value as Record<string, unknown> : undefined

function component(entity: Record<string, unknown>, names: string[]): [string, Record<string, unknown>] | undefined {
  for (const name of names) {
    const value = object(entity[name])
    if (value !== undefined) return [name, value]
  }
  return undefined
}

export function selectedPendingGltf(snapshot: Snapshot, selected: Iterable<string>, active: string | null): PendingGltf | undefined {
  const ids = active === null ? [...selected] : [active, ...[...selected].filter((id) => id !== active)]
  for (const entity of ids) {
    const values = snapshot[entity]
    if (values === undefined) continue
    const gltf = component(values, gltfNames)?.[1]
    const loading = component(values, loadingNames)?.[1]
    if (typeof gltf?.src === 'string' && loading?.currentState === 1) return { entity, src: gltf.src }
  }
  return undefined
}

export function applyPendingGltfLoadingState(snapshot: Snapshot, pending: PendingGltf, sampled: Snapshot): Snapshot {
  const current = snapshot[pending.entity]
  const candidate = sampled[pending.entity]
  if (current === undefined || candidate === undefined) return snapshot
  const currentGltf = component(current, gltfNames)?.[1]
  const candidateGltf = component(candidate, gltfNames)?.[1]
  const candidateLoading = component(candidate, loadingNames)
  if (currentGltf?.src !== pending.src || candidateGltf?.src !== pending.src || candidateLoading === undefined || typeof candidateLoading[1].currentState !== 'number') return snapshot
  return {
    ...snapshot,
    [pending.entity]: { ...current, [candidateLoading[0]]: { ...candidateLoading[1] } }
  }
}
