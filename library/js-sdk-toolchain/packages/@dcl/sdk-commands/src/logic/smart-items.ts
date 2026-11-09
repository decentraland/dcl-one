import { componentNumberFromName } from '@dcl/ecs/dist-cjs/components/component-number'

/** The component namespace composites carried before the smart-item runtime moved into the SDK. */
export const LEGACY_PREFIX = 'asset-packs::'
/** The component namespace of every smart-item component. */
export const PREFIX = 'smart-items::'
/** The build-time component listing the scripts attached to an entity. */
export const SCRIPT_COMPONENT = 'smart-items::Script'
/** The component whose value lists the component numbers an entity syncs. */
export const SYNC_COMPONENTS = 'core-schema::Sync-Components'

/** The smart-item component names (`BaseComponentNames` in the SDK's smart-items/constants.ts). */
const SMART_ITEM_COMPONENTS = [
  'ActionTypes',
  'Actions',
  'Counter',
  'Triggers',
  'States',
  'CounterBar',
  'AdminTools',
  'VideoScreen',
  'Rewards',
  'TextAnnouncements',
  'VideoControlState',
  'Script',
  'Placeholder'
]

/** The component number of every smart-item component under the legacy prefix, keyed to its current number. */
export const LEGACY_COMPONENT_IDS: ReadonlyMap<number, number> = new Map(
  SMART_ITEM_COMPONENTS.map((name) => [
    componentNumberFromName(LEGACY_PREFIX + name),
    componentNumberFromName(PREFIX + name)
  ])
)

/** Whether a component name belongs to the smart-item runtime, under either prefix. */
export function isEditorComponent(name: string): boolean {
  return name.startsWith(PREFIX) || name.startsWith(LEGACY_PREFIX)
}

/** Swaps the legacy prefix for the current one; names already renamed are returned as they are. */
export function renameLegacyComponentName(name: string): string {
  return name.startsWith(LEGACY_PREFIX) ? PREFIX + name.slice(LEGACY_PREFIX.length) : name
}

/** Maps the legacy component number of a smart-item component to its current one; other numbers are returned as they are. */
export function remapLegacyComponentId(id: number): number {
  return LEGACY_COMPONENT_IDS.get(id) ?? id
}

/** One component block of composite JSON: entity data keyed by entity string, each `{ json }`. */
export type CompositeBlock = {
  name: string
  jsonSchema?: unknown
  data?: Record<string, { json?: unknown } | undefined>
}

function remapSyncComponents(block: CompositeBlock): void {
  for (const entry of Object.values(block.data ?? {})) {
    const payload = entry?.json as { componentIds?: unknown[] } | undefined
    if (!payload || typeof payload !== 'object' || !Array.isArray(payload.componentIds)) continue
    payload.componentIds = payload.componentIds.map((id) => (typeof id === 'number' ? remapLegacyComponentId(id) : id))
  }
}

/** Folds `block` into `first`; `legacy` says whether each block carried the legacy prefix before renaming. */
function mergeBlock(first: { block: CompositeBlock; legacy: boolean }, block: CompositeBlock, legacy: boolean): void {
  const blockWins = first.legacy && !legacy
  const target = (first.block.data ??= {})
  for (const [entity, value] of Object.entries(block.data ?? {})) {
    if (blockWins || !(entity in target)) target[entity] = value
  }
  if (blockWins) {
    first.block.jsonSchema = block.jsonSchema ?? first.block.jsonSchema
    first.legacy = false
  } else {
    first.block.jsonSchema ??= block.jsonSchema
  }
}

/**
 * Renames every legacy component name of a composite in place and returns the composite.
 * The legacy component numbers listed by `core-schema::Sync-Components` become the current ones, and
 * blocks that end up with the same name are merged into the first: per entity the block that already
 * carried the current name wins, and so does its `jsonSchema` (else the first block that has one).
 */
export function renameLegacyComposite<T extends { components?: CompositeBlock[] }>(json: T): T {
  const components = json.components ?? []
  const merged: CompositeBlock[] = []
  const firstByName = new Map<string, { block: CompositeBlock; legacy: boolean }>()
  for (const block of components) {
    if (typeof block.name !== 'string') {
      merged.push(block)
      continue
    }
    const legacy = block.name.startsWith(LEGACY_PREFIX)
    block.name = renameLegacyComponentName(block.name)
    if (block.name === SYNC_COMPONENTS) remapSyncComponents(block)
    const first = firstByName.get(block.name)
    if (first) {
      mergeBlock(first, block, legacy)
    } else {
      firstByName.set(block.name, { block, legacy })
      merged.push(block)
    }
  }
  if (merged.length !== components.length) components.splice(0, components.length, ...merged)
  return json
}
