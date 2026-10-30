import { cmd } from '../cmd'
import { log } from '../log'
import {
  state,
  markEdited,
  markEntityDeleted,
  deleteSnapshotEntity,
  setSnapshotComponent
} from '../state'
import {
  isCustomComponent,
  customComponentId,
  customTimestamp,
  recordCustomTimestamp,
  encodeCustomComponent
} from '../custom-components'

export function applyLocalComponent(entityId: string, name: string, json: string): void {
  try {
    const value = JSON.parse(json) as unknown
    const existing = state.snapshot[entityId]?.[name]
    setSnapshotComponent(entityId, name, mergeKeepingOrder(existing, value))
  } catch {
  }
}

export function mergeKeepingOrder(existing: unknown, value: unknown): unknown {
  const isObj = (v: unknown): v is Record<string, unknown> =>
    v !== null && typeof v === 'object' && !Array.isArray(v)
  return isObj(existing) && isObj(value) ? { ...existing, ...value } : value
}

export function applyExternalComponentWrite(entityId: string, name: string, json: string): void {
  applyLocalComponent(entityId, name, json)
  try {
    markEdited(entityId, name, JSON.parse(json))
  } catch {
  }
}

export function applyExternalEntityDelete(entityId: string, recursive: boolean): void {
  removeLocal(entityId, recursive)
}

export async function writeComponent(entityId: string, name: string, json: string): Promise<void> {
  if (isCustomComponent(name)) {
    const id = customComponentId(name)
    const b64 = encodeCustomComponent(name, JSON.parse(json))
    if (id === undefined || b64 === undefined) {
      throw new Error(`cannot encode custom component ${name}`)
    }
    const ts = customTimestamp(entityId, name) + 1
    await cmd.setComponentRaw(entityId, id, ts, b64)
    recordCustomTimestamp(entityId, name, ts)
  } else {
    await cmd.setComponent(entityId, name, JSON.stringify(stripCase(JSON.parse(json))))
  }
  applyLocalComponent(entityId, name, json)
  markEdited(entityId, name, JSON.parse(json))
}

function removeLocal(id: string, recursive: boolean): void {
  if (!recursive) {
    deleteSnapshotEntity(id)
    markEntityDeleted(id)
    return
  }
  const all: string[] = []
  const stack = [id]
  while (stack.length > 0) {
    const cur = stack.pop() as string
    all.push(cur)
    for (const child of directChildren(cur)) stack.push(child)
  }
  for (const r of all) {
    deleteSnapshotEntity(r)
    markEntityDeleted(r)
  }
}

export async function writeDelete(id: string, recursive: boolean): Promise<void> {
  await cmd.deleteEntity(id, recursive)
  removeLocal(id, recursive)
}

export const isObj = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === 'object' && !Array.isArray(v)

export function stripCase(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stripCase)
  if (!isObj(value)) return value
  if (typeof value.$case === 'string' && value.$case in value) {
    const out: Record<string, unknown> = {}
    for (const [k, v] of Object.entries(value)) if (k !== '$case') out[k] = stripCase(v)
    return out
  }
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(value)) out[k] = stripCase(v)
  return out
}


const pendingTransforms = new Set<Promise<void>>()
let transformFailure: unknown
export async function flushTransforms(): Promise<void> {
  while (pendingTransforms.size) await Promise.all([...pendingTransforms])
  if (transformFailure !== undefined) {
    const failure = transformFailure
    transformFailure = undefined
    throw failure
  }
}

export function fireTransform(entityId: string, json: string): void {
  markEdited(entityId, 'Transform', JSON.parse(json))
  applyLocalComponent(entityId, 'Transform', json)
  const write = cmd.setComponent(entityId, 'Transform', json).then(() => {}, (error) => {
    transformFailure = error
    log.warn('gizmo transform write failed', entityId, error)
  })
  pendingTransforms.add(write)
  void write.then(() => pendingTransforms.delete(write))
}

type TransformValue = { parent: number }

function directChildren(id: string): string[] {
  const pid = Number(id)
  return Object.keys(state.snapshot).filter(
    (c) => (state.snapshot[c]?.Transform as TransformValue | undefined)?.parent === pid
  )
}
