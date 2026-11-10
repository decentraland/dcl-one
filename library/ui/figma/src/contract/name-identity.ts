import type { ParsedMapping } from './mapping-file.js'

/**
 * TODO(figma-identity): Align with the Design team on stable
 * component identity.
 *
 * Design's Cloud agents currently replace existing Figma
 * components by creating new ones and deleting the originals,
 * which changes their node IDs and component keys.
 *
 * Name-based resolution is a temporary workaround.
 *
 * Agree on a rule for Design agents to preserve existing
 * component nodes and update them in place whenever possible.
 * Once this workflow is enforced, revisit component identity
 * resolution and remove the temporary fallback if appropriate.
 */

type NameMatch = { mapping: ParsedMapping; matchedBy: 'component' | 'id' | 'alias' }

// Without a capture a Figma component set name can only be compared with what the mappings declare: the
// exported component name, the template id, or the code name figma.config.json gives a Figma set name.
// Every match is returned so the caller refuses an ambiguous name instead of picking one.
function matchMappingsByName(
  mappings: ParsedMapping[],
  name: string,
  componentNames: Readonly<Record<string, string>> = {}
): NameMatch[] {
  const alias = Object.hasOwn(componentNames, name) ? componentNames[name] : undefined
  const matches: NameMatch[] = []
  for (const mapping of mappings) {
    if (!mapping.figma) continue
    if (mapping.header.component === name) matches.push({ mapping, matchedBy: 'component' })
    else if (mapping.id === name) matches.push({ mapping, matchedBy: 'id' })
    else if (alias !== undefined && mapping.header.component === alias)
      matches.push({ mapping, matchedBy: 'alias' })
  }
  return matches
}

type Compatibility =
  | { ok: true }
  | { ok: false; reason: 'name-unconfirmed'; reads: string[] }
  | {
      ok: false
      reason: 'name-incompatible'
      unknown: string[]
      unsupported: { property: string; value: string; supported: string[] }[]
    }

// A name alone never makes the design's component and a mapping equivalent. The instance's variant
// properties must be ones the mapping reads (or declares as states), with values it supports; when the
// mapping reads variant properties, at least one must be given so there is something to compare.
function nameCompatibility(
  mapping: ParsedMapping,
  variantProperties: Readonly<Record<string, string>> = {}
): Compatibility {
  const enums = new Map(
    mapping.locals.flatMap((local) =>
      local.kind === 'enum' ? [[local.figmaProperty, Object.keys(local.values)] as const] : []
    )
  )
  const known = new Set([
    ...enums.keys(),
    ...mapping.locals.flatMap((local) => (local.kind === 'boolean' ? [local.figmaProperty] : [])),
    ...mapping.header.states
  ])
  const requested = Object.entries(variantProperties)
  if (requested.length === 0)
    return enums.size === 0
      ? { ok: true }
      : { ok: false, reason: 'name-unconfirmed', reads: [...enums.keys()] }
  const unknown = requested
    .filter(([property]) => !known.has(property))
    .map(([property]) => property)
  const unsupported = requested.flatMap(([property, value]) => {
    const supported = enums.get(property)
    return supported && !supported.includes(value) ? [{ property, value, supported }] : []
  })
  return unknown.length || unsupported.length
    ? { ok: false, reason: 'name-incompatible', unknown, unsupported }
    : { ok: true }
}

export { matchMappingsByName, nameCompatibility }
export type { Compatibility, NameMatch }
