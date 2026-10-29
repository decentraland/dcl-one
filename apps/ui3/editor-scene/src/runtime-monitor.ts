import { FIRST_SCENE_UPDATE_TICK, runtimeStatus } from './runtime-reset'
import type { LiveSceneInfo } from './bevy-api/interface'

export function replacementProjectScene(previous: LiveSceneInfo, scenes: LiveSceneInfo[]): LiveSceneInfo | undefined {
  const candidates = scenes.filter(scene => !scene.isPortable && !scene.isSuper)
  const same = candidates.find(scene => scene.hash === previous.hash)
  if (same) return same
  const parcels = new Set(previous.parcels.map(parcel => `${parcel.x},${parcel.y}`))
  const replacements = candidates.filter(scene => scene.parcels.length === parcels.size && scene.parcels.every(parcel => parcels.has(`${parcel.x},${parcel.y}`)))
  return replacements.length === 1 ? replacements[0] : undefined
}

export function createRuntimeMonitor(deps: { stats(): Promise<string>; changed(): Promise<void>; needsRecovery?(): boolean }) {
  let instance: string | undefined
  const monitor = async (hash: string, sampledStats?: string) => {
    const current = runtimeStatus(sampledStats ?? await deps.stats())
    if (current.hash !== hash) return
    if (current.broken) throw new Error('The updated scene failed to initialize')
    if (current.tick < FIRST_SCENE_UPDATE_TICK || current.inFlight) return
    const recovering = deps.needsRecovery?.() === true
    if (instance === undefined && !recovering) { instance = current.instance; return }
    if (instance === current.instance && !recovering) return
    await deps.changed()
    instance = current.instance
  }
  return Object.assign(monitor, { accept(stats: string) { instance = runtimeStatus(stats).instance } })
}
