import { cmd } from '../cmd'
import { state, markComponentDeleted, deleteSnapshotComponent } from '../state'
import { isCustomComponent, createCustomDefault, customComponentId, customTimestamp, recordCustomTimestamp } from '../custom-components'
import { loadSchema } from '../schema'
import { writeComponent } from './mutate'
import { reloadAfter } from './session'


export async function loadComponentNames(): Promise<void> {
  try {
    state.componentNames = await cmd.componentNames()
  } catch (e) {
    console.error('component_names failed:', e)
  }
}

export async function addComponent(entityId: string, name: string): Promise<void> {
  if (state.snapshot[entityId]?.[name] !== undefined) return

  let json = '{}'
  if (isCustomComponent(name)) {
    const value = createCustomDefault(name) ?? {}
    if (['asset-packs::Actions', 'asset-packs::States', 'asset-packs::Counter'].includes(name)) {
      ;(value as Record<string, unknown>).id = Number(entityId)
    }
    json = JSON.stringify(value)
  } else {
    const reply = await cmd.componentDefault(name)
    JSON.parse(reply)
    json = reply
  }

  await writeComponent(entityId, name, json)
  await reloadAfter()
  await loadSchema(name)
}

export async function deleteComponent(entityId: string, name: string): Promise<void> {
  const customId = customComponentId(name)
  await cmd.deleteComponent(entityId, String(customId ?? name))
  if (customId !== undefined) recordCustomTimestamp(entityId, name, customTimestamp(entityId, name) + 1)
  deleteSnapshotComponent(entityId, name)
  markComponentDeleted(entityId, name)
}

export async function setComponentValue(entityId: string, name: string, json: string): Promise<void> {
  let compact: string
  try {
    compact = JSON.stringify(JSON.parse(json))
  } catch {
    return
  }

  try {
    await writeComponent(entityId, name, compact)
    await reloadAfter()
  } catch (e) {
    console.error('set_component failed:', e)
  }
}
