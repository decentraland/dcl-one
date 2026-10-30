import { FIRST_SCENE_UPDATE_TICK, runtimeStatus } from './runtime-reset'

export function createProjectRead(deps: {
  stats(): Promise<string>
  refresh(): Promise<void>
  sleep(ms: number): Promise<void>
  now(): number
}) {
  return async <T>(hash: string, read: () => Promise<T>): Promise<T> => {
    const deadline = deps.now() + 10000
    while (deps.now() < deadline) {
      try {
        const before = runtimeStatus(await deps.stats())
        if (before.hash !== hash) throw new Error('The project scene is no longer selected')
        if (before.broken) throw new Error('The project scene failed to initialize')
        if (before.tick >= FIRST_SCENE_UPDATE_TICK && !before.inFlight) {
          await deps.refresh()
          const value = await read()
          const after = runtimeStatus(await deps.stats())
          if (after.hash === hash && after.instance === before.instance) return value
        }
      } catch (error) {
        if (!/active inspection scene no longer exists|scene has no thread handle/.test(String(error))) throw error
      }
      await deps.sleep(100)
    }
    throw new Error('Timed out waiting for the project reload to finish; no project data was exported')
  }
}
