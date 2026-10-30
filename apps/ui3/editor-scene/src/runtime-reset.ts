export interface RuntimeStatus {
  hash: string
  instance: string
  tick: number
  broken: boolean
  inFlight: boolean
}

export function runtimeStatus(stats: string): RuntimeStatus {
  const field = (name: string) => new RegExp(`^${name}: (.+)$`, 'm').exec(stats)?.[1]
  const hash = field('hash')
  const instance = field('instance')
  const tick = Number(field('tick'))
  if (!hash || !instance || !Number.isFinite(tick)) {
    throw new Error('The engine must expose scene_stats runtime instance identity to reset a scene safely')
  }
  return { hash, instance, tick, broken: field('broken') === 'true', inFlight: field('in_flight') === 'true' }
}

export interface ResetDependencies {
  stats(): Promise<string>
  reload(hash: string): Promise<unknown>
  pin(hash: string): Promise<unknown>
  freeze(): Promise<unknown>
  restore(composite: string): Promise<void>
  finish(): Promise<void>
  sleep(ms: number): Promise<void>
  now(): number
}

// Tick 1 initializes the runtime; tick 2 runs onStart/composite loading.
// The SDK schedules main() on the first onUpdate, which completes at tick 3.
export const FIRST_SCENE_UPDATE_TICK = 3

export function createRuntimeReset(deps: ResetDependencies) {
  return async (hash: string, composite: string): Promise<void> => {
    const parsed = JSON.parse(composite) as { components?: unknown }
    if (!parsed || !Array.isArray(parsed.components)) throw new Error('A scene composite is required')
    await deps.pin(hash)
    const previous = runtimeStatus(await deps.stats())
    if (previous.hash !== hash) throw new Error('The project scene could not be selected')
    await deps.reload(hash)
    const deadline = deps.now() + 30000
    let frozen = false
    while (deps.now() < deadline) {
      await deps.sleep(100)
      let current: RuntimeStatus
      try {
        await deps.pin(hash)
        current = runtimeStatus(await deps.stats())
      } catch {
        continue
      }
      if (current.hash !== hash || current.instance === previous.instance) continue
      if (current.broken) throw new Error('The restarted scene failed to initialize')
      if (current.tick < FIRST_SCENE_UPDATE_TICK) continue
      if (!frozen) {
        try {
          await deps.freeze()
        } catch (error) {
          if (!String(error).includes('already frozen')) throw error
        }
        frozen = true
        continue
      }
      if (current.inFlight) continue
      await deps.restore(composite)
      await deps.finish()
      return
    }
    throw new Error('Timed out waiting for the restarted project scene to initialize')
  }
}
