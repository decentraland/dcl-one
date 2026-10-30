import type { Snapshot } from './state'

export function createComponentBatch(deps: {
  ready(): Promise<void>
  snapshot(): Snapshot
  write(entity: string, name: string, value: unknown): Promise<void>
  remove(entity: string, name: string): Promise<void>
  refresh(): Promise<void>
}) {
  return async (entity: unknown, changes: unknown) => {
    if (typeof entity !== 'string' || !Number.isSafeInteger(Number(entity)) || (entity !== '0' && Number(entity) < 512)) throw new Error('Select an authored entity')
    if (!Array.isArray(changes) || !changes.length || changes.length > 100) throw new Error('Provide component changes')
    const names = new Set<string>()
    for (const change of changes) {
      if (!change || typeof change.name !== 'string' || !change.name || names.has(change.name) || !change.value || typeof change.value !== 'object' || Array.isArray(change.value)) throw new Error('Invalid component changes')
      names.add(change.name)
    }
    await deps.ready()
    const snapshot = deps.snapshot()
    if (!snapshot[entity]) throw new Error('The selected entity no longer exists')
    const before = new Map(changes.map(change => [change.name, snapshot[entity][change.name] === undefined ? undefined : JSON.parse(JSON.stringify(snapshot[entity][change.name]))]))
    const completed: string[] = []
    try {
      for (const change of changes) {
        await deps.write(entity, change.name, change.value)
        completed.push(change.name)
      }
    } catch (error) {
      let failed = false
      for (const name of completed.reverse()) {
        try {
          const value = before.get(name)
          if (value === undefined) await deps.remove(entity, name)
          else await deps.write(entity, name, value)
        } catch { failed = true }
      }
      if (failed) throw new Error(`The edit failed and some component values could not be restored. Reconnect and check the entity. ${String(error)}`)
      throw error
    }
    await deps.refresh()
    return changes.map(change => change.name)
  }
}
