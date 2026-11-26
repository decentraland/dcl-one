import path from 'node:path'
import { diagnostic, type Diagnostic } from './diagnostics.js'
import type { FigmaSet } from './figma-index.js'
import type { LibraryInfo } from './library.js'
import type { ParsedMapping } from './mapping-file.js'
import type { ResolveInput } from './resolve.js'

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

// The fallback is resolveByName and setsFittingVariantProperties in this file; resolve.ts calls them and adds
// the capture-side identity-name-match warning, and diagnostics.ts holds the name-* reasons.

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
  variantProperties: Readonly<Record<string, string>> = {},
  booleanProperties: Readonly<Record<string, string>> = {}
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
  // A state or boolean alone does not tell two components apart: when the mapping reads enum properties, at
  // least one of them must be given.
  if (enums.size > 0 && !requested.some(([property]) => enums.has(property)))
    return { ok: false, reason: 'name-unconfirmed', reads: [...enums.keys()] }
  if (requested.length === 0) return { ok: true }
  // A Figma BOOLEAN component property (booleanProperties) does not tell two components apart, so one the
  // mapping does not read is left to the unmapped-figma-property warning. One the mapping reads as a variant
  // is a different property, so it does not fit.
  const unknown = [
    ...requested.filter(([property]) => !known.has(property)).map(([property]) => property),
    ...Object.keys(booleanProperties).filter((property) => enums.has(property))
  ]
  const unsupported = requested.flatMap(([property, value]) => {
    const supported = enums.get(property)
    return supported && !supported.includes(value) ? [{ property, value, supported }] : []
  })
  return unknown.length || unsupported.length
    ? { ok: false, reason: 'name-incompatible', unknown, unsupported }
    : { ok: true }
}

// With a capture, several captured sets can share a name: the instance's variant properties may fit exactly
// one of them, which then identifies it; any other outcome leaves the name ambiguous.
function setsFittingVariantProperties(
  sets: FigmaSet[],
  variantProperties: Readonly<Record<string, string>> = {}
): FigmaSet[] {
  const requested = Object.entries(variantProperties)
  if (!requested.length) return []
  return sets.filter((set) =>
    requested.every(([property, value]) => {
      const definition = Object.hasOwn(set.properties, property)
        ? set.properties[property]
        : undefined
      return definition !== undefined && (!definition.options || definition.options.includes(value))
    })
  )
}

// Without a capture a set name resolves only to exactly one mapping, and only once the instance's variant
// properties fit it. Two mappings answering to one name are a library defect, never a choice.
function resolveByName(
  mappings: ParsedMapping[],
  name: string,
  input: ResolveInput,
  componentNames: Readonly<Record<string, string>> | undefined,
  library: LibraryInfo,
  context: Record<string, unknown>
): { errors: Diagnostic[] } | { setId: string; warning: Diagnostic } {
  const matches = matchMappingsByName(mappings, name, componentNames)
  if (matches.length === 0)
    return {
      errors: [
        diagnostic(
          'identity-uncertain',
          `No mapping declares the name "${name}" and no capture is available to identify it.`,
          { reason: 'name-unmatched', name, ...context }
        )
      ]
    }
  if (matches.length > 1)
    return {
      errors: [
        diagnostic('identity-ambiguous', `"${name}" matches ${matches.length} mappings.`, {
          reason: 'name-mappings',
          name,
          files: matches.map(({ mapping }) => path.relative(library.rootDir, mapping.file))
        })
      ]
    }
  const [{ mapping, matchedBy }] = matches as [(typeof matches)[number]]
  // A figma.config.json alias is a Figma set name (often a default such as "Component 1"), so it is no evidence on
  // its own: through an alias, a mapping that reads no variant property has nothing to confirm the name with.
  const readsVariants = mapping.locals.some((local) => local.kind === 'enum')
  if (matchedBy === 'alias' && !readsVariants)
    return {
      errors: [
        diagnostic(
          'identity-uncertain',
          `"${name}" is only an alias of ${mapping.header.component}, which reads no variant property to confirm it; resolve with the component name instead.`,
          {
            reason: 'name-unconfirmed',
            reads: [],
            name,
            component: mapping.header.component,
            ...context
          }
        )
      ]
    }
  const compatibility = nameCompatibility(mapping, input.variantProperties, input.booleanProperties)
  if (!compatibility.ok) {
    const details = Object.fromEntries(
      Object.entries(compatibility).filter(([key]) => key !== 'ok')
    )
    return {
      errors: [
        diagnostic(
          'identity-uncertain',
          compatibility.reason === 'name-unconfirmed'
            ? `"${name}" matches ${mapping.header.component}, which reads ${compatibility.reads.join(', ')}; pass the instance variant properties to confirm it.`
            : `The variant properties of "${name}" do not fit ${mapping.header.component}.`,
          {
            ...details,
            reason: compatibility.reason,
            name,
            component: mapping.header.component,
            ...context
          }
        )
      ]
    }
  }
  return {
    setId: mapping.figma!.nodeId,
    warning: diagnostic(
      'identity-name-match',
      `Identified "${name}" by name (${matchedBy}) as ${mapping.header.component}, checked against its variant properties.`,
      { name, matchedBy, component: mapping.header.component }
    )
  }
}

export { matchMappingsByName, nameCompatibility, resolveByName, setsFittingVariantProperties }
export type { Compatibility, NameMatch }
