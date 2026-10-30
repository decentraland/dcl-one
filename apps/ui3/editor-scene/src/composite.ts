
import * as ecs from '@dcl/sdk/ecs'
import { Composite } from '@dcl/sdk/ecs'
import { customComponentDefs, isCustomComponent } from './custom-components'
import { state } from './state'
import { hierarchyNodes } from './hierarchy-order'

type CompositeDef = { componentName: string; jsonSchema: unknown }

const DEFS = new Map<string, CompositeDef>()

function isComponentDef(
  v: unknown
): v is { componentName: string; schema: { jsonSchema: unknown } } {
  if (typeof v !== 'object' || v === null) return false
  const o = v as Record<string, unknown>
  const schema = o.schema as Record<string, unknown> | undefined
  return (
    typeof o.componentName === 'string' &&
    typeof schema === 'object' &&
    schema !== null &&
    'jsonSchema' in schema
  )
}

for (const [key, val] of Object.entries(ecs as Record<string, unknown>)) {
  if (isComponentDef(val)) {
    DEFS.set(key, { componentName: val.componentName, jsonSchema: val.schema.jsonSchema })
  }
}
for (const d of customComponentDefs()) {
  DEFS.set(d.componentName, d)
}

const SNAPSHOT_NAME_BY_COMPOSITE = new Map<string, string>()
for (const [snapshotName, def] of DEFS) {
  SNAPSHOT_NAME_BY_COMPOSITE.set(def.componentName, snapshotName)
}
export function compositeToSnapshotName(compositeName: string): string | undefined {
  return SNAPSHOT_NAME_BY_COMPOSITE.get(compositeName)
}

type AuthoredData = Record<string, Record<string, unknown>>

export function isAuthoredEntity(eid: number): boolean {
  return eid === 0 || eid >= 512
}

export function isSavableComponent(name: string): boolean {
  if (!DEFS.has(name)) return false
  return isCustomComponent(name) || state.componentNames.includes(name)
}

const INSPECTOR_NODES = 'inspector::Nodes'

export function buildComposite(authored: AuthoredData): string {
  type Comp = {
    name: string
    jsonSchema: unknown
    data: Map<number, { data: { $case: 'json'; json: unknown } }>
  }
  const byComponent = new Map<string, Comp>()

  for (const [entityId, comps] of Object.entries(authored)) {
    const eid = Number(entityId)
    if (!Number.isFinite(eid) || !isAuthoredEntity(eid)) continue
    for (const [name, value] of Object.entries(comps)) {
      if (!isSavableComponent(name)) continue
      const def = DEFS.get(name)
      if (def === undefined) continue
      let comp = byComponent.get(def.componentName)
      if (comp === undefined) {
        comp = { name: def.componentName, jsonSchema: def.jsonSchema, data: new Map() }
        byComponent.set(def.componentName, comp)
      }
      comp.data.set(eid, { data: { $case: 'json', json: value } })
    }
  }

  const nodesDef = DEFS.get(INSPECTOR_NODES)
  if (nodesDef !== undefined) {
    let comp = byComponent.get(nodesDef.componentName)
    if (comp === undefined) {
      comp = { name: nodesDef.componentName, jsonSchema: nodesDef.jsonSchema, data: new Map() }
      byComponent.set(nodesDef.componentName, comp)
    }
    comp.data.set(0, { data: { $case: 'json', json: { value: hierarchyNodes(authored) } } })
  }

  const definition = {
    version: 1,
    components: [...byComponent.values()]
  } as unknown as Composite.Definition

  return JSON.stringify(Composite.toJson(definition))
}
