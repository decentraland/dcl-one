import { FIRST_SCENE_UPDATE_TICK, runtimeStatus } from './runtime-reset'

export const AUTHORING_RPCS = new Set([
  'moveHierarchy', 'pasteEntities', 'pasteComponent', 'writeComponents',
  'writeMaterials', 'removeComponent', 'removeEntities', 'addEntity', 'addComponent'
])
export const AUTHORING_MESSAGES = new Set([
  'add-entity', 'set-component', 'add-component', 'delete-component',
  'entity-deleted', 'component-written', 'load-scene'
])

/** Export before reporting an edit as successful. A watcher can replace the
 * project immediately after that acknowledgement: a later page-side export
 * would then capture the replacement, not the edit we just acknowledged. */
export function createAuthoringCheckpoint(deps: {
  project(): { hash: string; playing: boolean } | null
  stats(): Promise<string>
  flush(): Promise<void>
  read(): Promise<string>
  send(checkpoint: { composite: string; runtime: string }): void
}) {
  return async <T>(edit: () => T | Promise<T>): Promise<T> => {
    const project = deps.project()
    if (!project || project.playing) throw new Error('Stop playback before editing the project')
    const before = runtimeStatus(await deps.stats())
    const identity = `${before.hash}:${before.instance}`
    const healthy = (value: typeof before) => value.hash === project.hash &&
      value.tick >= FIRST_SCENE_UPDATE_TICK && !value.broken && !value.inFlight
    if (!healthy(before)) throw new Error('The authored scene is not ready; retry after it reconnects')
    const verify = async () => {
      const active = deps.project()
      const after = runtimeStatus(await deps.stats())
      if (!active || active.playing || active.hash !== project.hash ||
          !healthy(after) || `${after.hash}:${after.instance}` !== identity) {
        throw new Error('The authored scene changed during the edit; its result could not be checkpointed')
      }
    }
    const result = await edit()
    await deps.flush()
    await verify()
    const composite = await deps.read()
    const parsed = JSON.parse(composite) as { components?: unknown } | null
    if (!parsed || !Array.isArray(parsed.components)) throw new Error('The authored scene export was invalid')
    await verify()
    deps.send({ composite, runtime: identity })
    return result
  }
}
