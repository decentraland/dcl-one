import { Schemas } from '@dcl/ecs'
import { componentNumberFromName } from '@dcl/ecs/dist/components/component-number'
import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer'
import { BaseComponentNames } from './constants'

/** The component namespace composites carried before the runtime became @dcl/smart-items. */
export const LEGACY_PREFIX = 'asset-packs::'
/** The component namespace of every smart-item component. */
export const PREFIX = 'smart-items::'
/** The component whose value lists the component numbers an entity syncs. */
export const SYNC_COMPONENTS = 'core-schema::Sync-Components'
/** The schema of `core-schema::Sync-Components`, for the entries a binary composite carries. */
const SYNC_COMPONENTS_SCHEMA = Schemas.Map({ componentIds: Schemas.Array(Schemas.Int64) })
/** A trigger's reference to a component of its own entity (`{self:…}`) or of another one (`{<id>:…}`). */
const LEGACY_PLACEHOLDER = /^\{(self|\d+):asset-packs::(.+)\}$/
/** Composite JSON keys entity data by the entity number. */
const ENTITY_KEY = /^\d+$/

/** Swaps the legacy prefix for the current one; names already renamed are returned as they are. */
export function renameLegacyComponentName(name: string): string {
  return name.startsWith(LEGACY_PREFIX) ? PREFIX + name.slice(LEGACY_PREFIX.length) : name
}

/** Renames the component a trigger placeholder names (`{self:asset-packs::Actions}`); other strings are returned as they are. */
export function renameLegacyPlaceholder(value: string): string {
  const match = LEGACY_PLACEHOLDER.exec(value)
  return match ? `{${match[1]}:${PREFIX}${match[2]}}` : value
}

/** The component number of every smart-item component under the legacy prefix, keyed to its current number. */
export const LEGACY_COMPONENT_IDS: ReadonlyMap<number, number> = new Map(
  Object.values(BaseComponentNames).map((name) => [
    componentNumberFromName(LEGACY_PREFIX + name.slice(PREFIX.length)),
    componentNumberFromName(name)
  ])
)

/** Maps the legacy component number of a smart-item component to its current one; other numbers are returned as they are. */
export function remapLegacyComponentId(id: number): number {
  return LEGACY_COMPONENT_IDS.get(id) ?? id
}

/**
 * One component block of a composite, either as `Composite.Definition` (entity data in a Map of
 * `{ data: { $case: 'json', json } }` or `{ data: { $case: 'binary', binary } }`) or as composite JSON
 * (entity data keyed by entity string, `{ json }`).
 */
export type CompositeBlock = {
  name: string
  jsonSchema?: unknown
  data?: Map<number, unknown> | Record<string, unknown>
}

type EntityData = Map<number, unknown> | Record<string, unknown>

type Entry = {
  data?: { $case?: string; json?: unknown; binary?: Uint8Array }
  json?: unknown
  binary?: unknown
}

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

/** Decodes the base64 a composite JSON file carries for a binary entry; the runtime has no Buffer or atob. */
function bytesFromBase64(text: string): Uint8Array {
  const chars = text.replace(/[^A-Za-z0-9+/]/g, '')
  const bytes = new Uint8Array(Math.floor((chars.length * 3) / 4))
  let bits = 0
  let buffer = 0
  let length = 0
  for (const char of chars) {
    buffer = (buffer << 6) | BASE64.indexOf(char)
    bits += 6
    if (bits >= 8) {
      bits -= 8
      bytes[length++] = (buffer >> bits) & 0xff
    }
  }
  return bytes.subarray(0, length)
}

function base64FromBytes(bytes: Uint8Array): string {
  let text = ''
  for (let i = 0; i < bytes.length; i += 3) {
    const chunk = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0)
    text += BASE64[(chunk >> 18) & 63] + BASE64[(chunk >> 12) & 63]
    text += i + 1 < bytes.length ? BASE64[(chunk >> 6) & 63] : '='
    text += i + 2 < bytes.length ? BASE64[chunk & 63] : '='
  }
  return text
}

/** Drops the keys of a composite JSON block that are not entity numbers (`__proto__` and friends included). */
function pruneEntityKeys(data: EntityData | undefined): void {
  if (!data || data instanceof Map) return
  for (const key of Object.keys(data)) if (!ENTITY_KEY.test(key)) delete data[key]
}

function entriesOf(data: EntityData | undefined): [number | string, unknown][] {
  if (!data) return []
  if (data instanceof Map) return [...data.entries()]
  return Object.entries(data).filter(([key]) => ENTITY_KEY.test(key))
}

function setEntry(data: EntityData, key: number | string, value: unknown, overwrite: boolean): void {
  if (data instanceof Map) {
    if (overwrite || !data.has(key as number)) data.set(key as number, value)
  } else if (overwrite || !Object.prototype.hasOwnProperty.call(data, key)) {
    data[key] = value
  }
}

function jsonPayload(entry: unknown): Record<string, unknown> | undefined {
  if (!entry || typeof entry !== 'object') return undefined
  const { data, json } = entry as Entry
  const payload = data?.$case === 'json' ? data.json : json
  return payload && typeof payload === 'object' ? (payload as Record<string, unknown>) : undefined
}

/** Remaps the component numbers of one serialized `core-schema::Sync-Components` value. */
function remapSyncComponentBytes(bytes: Uint8Array): Uint8Array {
  const value = SYNC_COMPONENTS_SCHEMA.deserialize(new ReadWriteByteBuffer(bytes))
  value.componentIds = value.componentIds.map(remapLegacyComponentId)
  const buffer = new ReadWriteByteBuffer()
  SYNC_COMPONENTS_SCHEMA.serialize(value, buffer)
  return buffer.toBinary()
}

/** A `Composite.Definition` entry holds the bytes of a binary entry; composite JSON holds them as base64. */
function remapSyncComponentsEntry(entry: unknown): void {
  if (!entry || typeof entry !== 'object') return
  const { data, binary } = entry as Entry
  if (data?.$case === 'binary' && data.binary) {
    data.binary = remapSyncComponentBytes(data.binary)
  } else if (typeof binary === 'string') {
    ;(entry as Entry).binary = base64FromBytes(remapSyncComponentBytes(bytesFromBase64(binary)))
  } else {
    const payload = jsonPayload(entry)
    if (!payload || !Array.isArray(payload.componentIds)) return
    payload.componentIds = payload.componentIds.map((id) => (typeof id === 'number' ? remapLegacyComponentId(id) : id))
  }
}

function remapSyncComponents(block: CompositeBlock): void {
  for (const [, entry] of entriesOf(block.data)) remapSyncComponentsEntry(entry)
}

function renamePlaceholders(value: unknown): void {
  if (!value || typeof value !== 'object') return
  const record = value as Record<string, unknown>
  for (const key of Object.keys(record)) {
    const item = record[key]
    if (typeof item === 'string') record[key] = renameLegacyPlaceholder(item)
    else renamePlaceholders(item)
  }
}

/** Catalog triggers reference the actions and states of their item through placeholders that name the component. */
function renameTriggerPlaceholders(block: CompositeBlock): void {
  for (const [, entry] of entriesOf(block.data)) {
    const payload = jsonPayload(entry)
    if (payload) renamePlaceholders(payload)
  }
}

/** Folds `block` into `first`; `legacy` says whether each block carried the legacy prefix before renaming. */
function mergeBlock(first: { block: CompositeBlock; legacy: boolean }, block: CompositeBlock, legacy: boolean): void {
  const blockWins = first.legacy && !legacy
  const target = (first.block.data ??= block.data instanceof Map ? new Map() : {})
  for (const [key, value] of entriesOf(block.data)) setEntry(target, key, value, blockWins)
  if (blockWins) {
    first.block.jsonSchema = block.jsonSchema ?? first.block.jsonSchema
    first.legacy = false
  } else {
    first.block.jsonSchema ??= block.jsonSchema
  }
}

/**
 * Renames every legacy component name of a composite in place and returns the composite.
 * The legacy component numbers listed by `core-schema::Sync-Components` become the current ones (binary
 * entries included), the placeholders of `Triggers` name the current components, and blocks that end up
 * with the same name are merged into the first: per entity the block that already carried the current
 * name wins, and so does its `jsonSchema` (else the first block that has one). Entity keys that are not
 * numbers are dropped.
 */
export function renameLegacyComposite<T extends { components?: CompositeBlock[] }>(json: T): T {
  const components = json.components ?? []
  const merged: CompositeBlock[] = []
  const firstByName = new Map<string, { block: CompositeBlock; legacy: boolean }>()
  for (const block of components) {
    const legacy = block.name.startsWith(LEGACY_PREFIX)
    block.name = renameLegacyComponentName(block.name)
    pruneEntityKeys(block.data)
    if (block.name === SYNC_COMPONENTS) remapSyncComponents(block)
    if (block.name === BaseComponentNames.TRIGGERS) renameTriggerPlaceholders(block)
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
