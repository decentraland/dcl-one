import { cmd } from './cmd'
import { log } from './log'
import { state, parentOf, setSchema } from './state'

export type EnumValues = Array<[string, number]>

export type SchemaNode =
  | { name?: string; kind: 'message'; fields: SchemaNode[]; optional?: boolean }
  | { name?: string; kind: 'oneof'; cases: Array<{ name: string; field: SchemaNode }> }
  | { name?: string; kind: 'repeated'; element: SchemaNode; optional?: boolean }
  | {
      name?: string
      kind: 'leaf'
      semantic: string
      enum?: string
      optional?: boolean
      default?: unknown
      range?: { min?: number; max?: number; hard: boolean }
      notes?: string
    }

export type ComponentSchema = {
  name: string
  placement: string
  readOnly: boolean
  requires: Array<{ component: string; locality: string; hard: boolean }>
  root: SchemaNode
  enums: Record<string, EnumValues>
}

export function getSchema(name: string): ComponentSchema | undefined {
  return state.schemas.get(name) as ComponentSchema | undefined
}

export async function loadSchema(name: string): Promise<void> {
  if (state.schemas.has(name)) return
  try {
    const reply = await cmd.componentSchema(name)
    setSchema(name, JSON.parse(reply))
  } catch {
  }
}

export function toSdkValue(value: unknown, node: SchemaNode): unknown {
  if (value === null || value === undefined) return value
  switch (node.kind) {
    case 'oneof': {
      if (typeof value !== 'object' || Array.isArray(value)) return value
      const obj = value as Record<string, unknown>
      if (typeof obj.$case === 'string') {
        const active = node.cases.find((c) => c.name === obj.$case)
        return active === undefined
          ? value
          : { $case: obj.$case, [obj.$case]: toSdkValue(obj[obj.$case], active.field) }
      }
      for (const c of node.cases) {
        const cv = obj[c.name]
        if (cv !== null && cv !== undefined) {
          return { $case: c.name, [c.name]: toSdkValue(cv, c.field) }
        }
      }
      return null
    }
    case 'message': {
      if (typeof value !== 'object' || Array.isArray(value)) return value
      const obj = value as Record<string, unknown>
      const out: Record<string, unknown> = {}
      for (const f of node.fields) {
        const k = f.name as string
        const v = toSdkValue(obj[k], f)
        if (v !== null && v !== undefined) out[k] = v
      }
      return out
    }
    case 'repeated': {
      if (!Array.isArray(value)) return value
      return value.map((v) => toSdkValue(v, node.element))
    }
    default:
      return value
  }
}

export function ensureSchema(name: string): void {
  if (state.schemas.has(name) || state.schemaPending.has(name)) return
  state.schemaPending.add(name)
  cmd.componentSchema(name)
    .then((reply) => {
      try {
        setSchema(name, JSON.parse(reply))
      } catch {
      }
    })
    .catch((e) => log.warn('component schema fetch failed', name, e))
    .then(() => {
      state.schemaPending.delete(name)
    })
}

function hasComponentAt(entityId: string, component: string, locality: string): boolean {
  const has = (eid: string): boolean => component in (state.snapshot[eid] ?? {})
  const parent = (eid: string): string | null => parentOf(state.snapshot, eid)
  switch (locality) {
    case 'same':
      return has(entityId)
    case 'parent': {
      const p = parent(entityId)
      return p !== null && has(p)
    }
    case 'ancestor': {
      const seen = new Set<string>([entityId])
      for (let p = parent(entityId); p !== null && !seen.has(p); p = parent(p)) {
        if (has(p)) return true
        seen.add(p)
      }
      return false
    }
    default:
      return true
  }
}

export function restrictionUnmet(name: string, entityId: string): string | null {
  const schema = getSchema(name)
  if (schema === undefined) return null

  if (schema.placement === 'root' && entityId !== '0') return 'scene root only'

  for (const req of schema.requires) {
    if (req.hard && !hasComponentAt(entityId, req.component, req.locality)) {
      return `needs ${req.component}`
    }
  }
  return null
}
