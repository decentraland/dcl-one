// Token mappings of linked items: which tokens of the collection's linked contract unlock the item, ported from the
// legacy builder's MappingEditor + modules/item/utils (getMapping / buildItemMappings / areMappingsValid).
import { MappingType, Mappings, type ContractNetwork, type Mapping } from '@dcl/schemas'
import { type LinkedContract } from './thirdParty'

export { MappingType }
export type { Mapping, Mappings }

/** The editor's form state: every value as typed, validated into a `Mapping` on save. */
export type MappingDraft = {
  type: MappingType
  /** `single`: the token id. */
  id: string
  /** `multiple`: the token ids as typed (commas, spaces or newlines between them). */
  ids: string
  /** `range`: both bounds. */
  from: string
  to: string
}

export const EMPTY_MAPPING_DRAFT: MappingDraft = { type: MappingType.ANY, id: '', ids: '', from: '', to: '' }

export const DEFAULT_MAPPING: Mapping = { type: MappingType.ANY }

// No leading zeros: the schema matches single and multiple ids as exact strings, so `007` would never match token 7.
const TOKEN_ID = /^(0|[1-9]\d*)$/
const MAX_TOKEN_ID = 2n ** 256n - 1n
// uint256 fits in 78 decimal digits.
export const TOKEN_ID_MAX_LENGTH = 78

/** The item's first mapping for the collection's linked contract (legacy getMapping), or null. */
export function getItemMapping(mappings: Mappings | null | undefined, contract: LinkedContract | null): Mapping | null {
  if (!mappings || !contract) return null
  const byAddress = mappings[contract.network]
  if (!byAddress) return null
  const entry = Object.entries(byAddress).find(([address]) => address.toLowerCase() === contract.address.toLowerCase())
  return entry?.[1][0] ?? null
}

/** Wraps a mapping under the collection's contract, the only shape builder-server accepts (one network, one address). */
export function buildItemMappings(mapping: Mapping, contract: LinkedContract): Mappings {
  return { [contract.network]: { [contract.address.toLowerCase()]: [mapping] } }
}

/**
 * The item's mapping moved under `to`, wherever it sits now (a retry finds items already moved and items still under
 * the old contract alike); null when the item is unmapped or already under `to`.
 */
export function relinkMappings(mappings: Mappings | null | undefined, to: LinkedContract): Mappings | null {
  if (!mappings || getItemMapping(mappings, to)) return null
  const mapping = Object.values(mappings).flatMap(byAddress => Object.values(byAddress ?? {}))[0]?.[0]
  return mapping ? buildItemMappings(mapping, to) : null
}

/** `007` → `7`: the schema matches ids as exact strings, so a padded id would never match its token. */
export function normalizeTokenId(value: string): string {
  const trimmed = value.trim()
  return /^\d+$/.test(trimmed) ? trimmed.replace(/^0+(?=\d)/, '') : trimmed
}

export function parseTokenIds(text: string): string[] {
  return text
    .split(/[\s,;]+/)
    .map(normalizeTokenId)
    .filter(part => part.length > 0)
}

/** The mapping with its token ids normalized, for stored or imported data written before ids were checked. */
export function normalizeMapping(mapping: Mapping): Mapping {
  switch (mapping.type) {
    case MappingType.SINGLE:
      return { ...mapping, id: normalizeTokenId(mapping.id) }
    case MappingType.MULTIPLE:
      return { ...mapping, ids: mapping.ids.map(normalizeTokenId) }
    case MappingType.RANGE:
      return { ...mapping, from: normalizeTokenId(mapping.from), to: normalizeTokenId(mapping.to) }
    default:
      return mapping
  }
}

export function toMappingDraft(mapping: Mapping | null): MappingDraft {
  if (!mapping) return EMPTY_MAPPING_DRAFT
  switch (mapping.type) {
    case MappingType.SINGLE:
      return { ...EMPTY_MAPPING_DRAFT, type: mapping.type, id: mapping.id }
    case MappingType.MULTIPLE:
      return { ...EMPTY_MAPPING_DRAFT, type: mapping.type, ids: mapping.ids.join(', ') }
    case MappingType.RANGE:
      return { ...EMPTY_MAPPING_DRAFT, type: mapping.type, from: mapping.from, to: mapping.to }
    default:
      return { ...EMPTY_MAPPING_DRAFT, type: MappingType.ANY }
  }
}

/** The mapping the draft describes, or null while it is incomplete or invalid. */
export function fromMappingDraft(draft: MappingDraft): Mapping | null {
  let mapping: Mapping
  switch (draft.type) {
    case MappingType.ANY:
      mapping = { type: MappingType.ANY }
      break
    case MappingType.SINGLE:
      mapping = { type: MappingType.SINGLE, id: normalizeTokenId(draft.id) }
      break
    case MappingType.MULTIPLE:
      mapping = { type: MappingType.MULTIPLE, ids: parseTokenIds(draft.ids) }
      break
    case MappingType.RANGE:
      mapping = { type: MappingType.RANGE, from: normalizeTokenId(draft.from), to: normalizeTokenId(draft.to) }
      break
    default:
      return null
  }
  return isMappingValid(mapping) ? mapping : null
}

/** Whether a single mapping is well formed: uint256 token ids without leading zeros, unique ids, `from <= to`. */
export function isMappingValid(mapping: Mapping): boolean {
  switch (mapping.type) {
    case MappingType.ANY:
      return true
    case MappingType.SINGLE:
      return isTokenId(mapping.id)
    case MappingType.MULTIPLE:
      return mapping.ids.length > 0 && mapping.ids.every(isTokenId) && new Set(mapping.ids).size === mapping.ids.length
    case MappingType.RANGE:
      return isTokenId(mapping.from) && isTokenId(mapping.to) && BigInt(mapping.from) <= BigInt(mapping.to)
    default:
      return false
  }
}

function isTokenId(value: string): boolean {
  return (
    typeof value === 'string' &&
    value.length <= TOKEN_ID_MAX_LENGTH &&
    TOKEN_ID.test(value) &&
    BigInt(value) <= MAX_TOKEN_ID
  )
}

/** The schema's own check over the whole payload (legacy areMappingsValid), for what came from a zip or the server. */
export function areMappingsValid(mappings: unknown): mappings is Mappings {
  return Mappings.validate(mappings) === true
}

export function areMappingsEqual(a: Mappings | null | undefined, b: Mappings | null | undefined): boolean {
  return JSON.stringify(a ?? null) === JSON.stringify(b ?? null)
}

/** A `wearable.json` mapping as the docs describe it, wrapped under the collection's contract; null when unusable. */
export function mappingsFromManifest(mapping: unknown, contract: LinkedContract | null): Mappings | null {
  if (!contract || !mapping || typeof mapping !== 'object') return null
  const raw = buildItemMappings(mapping as Mapping, contract)
  if (!areMappingsValid(raw)) return null
  const normalized = normalizeMapping(mapping as Mapping)
  return isMappingValid(normalized) ? buildItemMappings(normalized, contract) : null
}

export type MappingLabel = { id: string; values?: Record<string, string | number> }

/**
 * The item's token mappings for the collection's linked contract, one message per rule, naming the tokens themselves
 * (never just a count) so a reviewer can check them.
 */
export function getMappingLabels(
  mappings: unknown,
  network: string | undefined,
  contractAddress: string | undefined
): MappingLabel[] {
  const key = (name: string) => `linked_collection.mapping.${name}`
  const none = [{ id: key('none') }]
  if (!mappings || typeof mappings !== 'object' || !network || !contractAddress) return none
  const contracts = (mappings as Mappings)[network as ContractNetwork]
  const entries = contracts
    ? Object.entries(contracts).find(([address]) => address.toLowerCase() === contractAddress.toLowerCase())?.[1]
    : undefined
  if (!Array.isArray(entries) || entries.length === 0) return none
  return entries.map((mapping): MappingLabel => {
    switch (mapping.type) {
      case MappingType.ANY:
        return { id: key('any') }
      case MappingType.SINGLE:
        return { id: key('single'), values: { id: mapping.id } }
      case MappingType.RANGE:
        return { id: key('range'), values: { from: mapping.from, to: mapping.to } }
      case MappingType.MULTIPLE:
        return { id: key('multiple'), values: { ids: mapping.ids.map(id => `#${id}`).join(', ') } }
      default:
        return { id: key('none') }
    }
  })
}
