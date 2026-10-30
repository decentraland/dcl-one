import { createComponentBatch } from './component-batch'

type Dependencies = Parameters<typeof createComponentBatch>[0] & { context?(): Promise<string> }
type Change = { entity: string; name: string; value: Record<string, unknown>; before: Record<string, unknown> }
const object = (value: unknown): value is Record<string, unknown> => Boolean(value) && typeof value === 'object' && !Array.isArray(value)
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
  if (object(value)) return `{${Object.keys(value).filter(key => key !== '$case' && value[key] !== undefined).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`
  return JSON.stringify(value)
}

export function createMaterialBatch(deps: Dependencies) {
  const writeBatch = createComponentBatch({ ...deps, ready: async () => {}, refresh: async () => {} })
  let pending = false
  return async (input: unknown) => {
    if (pending) throw new Error('Wait for the current material edit to finish')
    if (!Array.isArray(input) || input.length < 2 || input.length > 100) throw new Error('Select between 2 and 100 materials')
    const entities = new Set<string>()
    for (const change of input) {
      if (!object(change) || typeof change.entity !== 'string' || !/^\d+$/.test(change.entity) || !Number.isSafeInteger(Number(change.entity)) || Number(change.entity) < 512 || entities.has(change.entity) || !['Material', 'core::Material'].includes(String(change.name)) || !object(change.value) || !object(change.before)) throw new Error('Invalid material selection')
      entities.add(change.entity)
    }
    const changes = JSON.parse(JSON.stringify(input)) as Change[]
    pending = true
    const completed: Change[] = []
    const deadline = Date.now() + 60000
    try {
      await deps.ready()
      const identity = await deps.context?.()
      const assertContext = async (until = deadline) => {
        if (Date.now() >= until) throw new Error('The material edit exceeded its time budget')
        if (deps.context && await deps.context() !== identity) throw new Error('The project runtime changed during the material edit')
      }
      const assertBaseline = (change: Change, expected: unknown) => {
        if (canonical(deps.snapshot()[change.entity]?.[change.name]) !== canonical(expected)) throw new Error('A selected material changed during the edit')
      }
      const snapshot = deps.snapshot()
      for (const change of changes) {
        const current = snapshot[change.entity]?.[change.name]
        if (!object(current)) throw new Error('Every selected entity must still have a Material component')
        if (canonical(current) !== canonical(change.before)) throw new Error('A selected material changed. Discard the draft and try again')
        change.before = JSON.parse(JSON.stringify(current)) as Record<string, unknown>
      }
      try {
        for (const change of changes) {
          await assertContext()
          assertBaseline(change, change.before)
          await writeBatch(change.entity, [{ name: change.name, value: change.value }])
          completed.push(change)
        }
        await assertContext()
        await deps.refresh()
      } catch (error) {
        let failed = false
        const rollbackDeadline = Date.now() + 20000
        for (const change of completed.reverse()) {
          try {
            await assertContext(rollbackDeadline)
            assertBaseline(change, change.value)
            await writeBatch(change.entity, [{ name: change.name, value: change.before }])
          }
          catch { failed = true }
        }
        if (failed) throw new Error(`The edit failed and some materials could not be restored. Reconnect and check the selection. ${String(error)}`)
        throw error
      }
      return changes.map(change => change.entity)
    } finally { pending = false }
  }
}
