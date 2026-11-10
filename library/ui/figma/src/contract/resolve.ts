import path from 'node:path'
import { diagnostic, type Diagnostic } from './diagnostics.js'
import { normalizeNodeId, type FigmaSet } from './figma-index.js'
import { findMappingFiles, loadLibrary, readCaptureRevision, type LibraryInfo } from './library.js'
import { readMappingFile, type MappingLocal, type ParsedMapping } from './mapping-file.js'
import { importSpecifier, validateAgainstFigma, validateAgainstLibrary } from './validate.js'

type ResolveContext = {
  mappingRoot?: string
  libraryRange?: string
  // Empty when no capture is available; identity is then checked against the mapping only.
  figmaSets: FigmaSet[]
  // Synced components promotion.include names and promotion has not written yet, with the reasons
  // promotion would skip each one (none when eligible). Any other hand-written mapping is validated.
  loadSyncedComponents?: () => Promise<Map<string, string[]>>
}

type ResolveInput = {
  nodeId?: string
  name?: string
  variantProperties?: Record<string, string>
  texts?: Record<string, string>
}

type PropTranslation = {
  figmaProperty: string
  figmaType: 'VARIANT' | 'BOOLEAN' | 'TEXT'
  codeProp?: string
  values?: { figma: string; code: string }[]
  selected?: { figma: string; code: string }
  note?: string
}

type Resolved = {
  component: { exportName: string; importSpecifier: string; importStatements: string[] }
  usage: string
  usageFor: { variantProperties: Record<string, string>; basis: 'requested' | 'figma-defaults' }
  propTranslation: PropTranslation[]
  interactionStates: string[]
  provenance: {
    figma: {
      fileKey: string
      setId: string
      setName?: string
      page?: string
      variantNodeId?: string
      url: string
      identity: 'verified-against-capture' | 'mapping-only'
    }
    mapping: { file: string; id?: string; contract: string }
    library: { name: string; version: string }
    // The capture revision the library's tracked lock pins, in both modes; absent when the library has no lock.
    capture?: { revision: string }
  }
}

type ResolveResult =
  | { ok: true; resolved: Resolved; warnings: Diagnostic[] }
  | { ok: false; errors: Diagnostic[]; warnings: Diagnostic[] }

const fail = (errors: Diagnostic[], warnings: Diagnostic[] = []): ResolveResult => ({
  ok: false,
  errors,
  warnings
})

function codePropOf(local: MappingLocal, example: string): string | undefined {
  if (local.kind === 'text') return undefined
  const attribute = new RegExp(
    `(\\w[\\w-]*)=(?:"\\$\\{${local.local}\\}"|\\{\\$\\{${local.local}\\}\\})`
  ).exec(example)
  if (attribute) return attribute[1]
  if (new RegExp(`\\$\\{${local.local}\\}`).test(example)) {
    if (local.kind === 'string') return undefined
    const bare =
      local.kind === 'enum' ? Object.values(local.values) : [local.whenTrue, local.whenFalse]
    const names = [...new Set(bare.map((value) => value.trim()).filter(Boolean))]
    if (names.length === 1) return names[0]
  }
  return undefined
}

function variantDefaults(set: FigmaSet): Record<string, string> {
  const defaults: Record<string, string> = {}
  for (const [name, property] of Object.entries(set.properties)) {
    if (property.type === 'VARIANT' && property.default !== undefined) {
      defaults[name] = String(property.default)
    }
  }
  return defaults
}

function comboExists(set: FigmaSet, requested: Record<string, string>): boolean {
  const defaults = variantDefaults(set)
  return set.variants.some((variant) =>
    Object.keys(defaults).every(
      (name) => (variant.props[name] ?? defaults[name]) === requested[name]
    )
  )
}

function nearestVariants(set: FigmaSet, requested: Record<string, string>) {
  const defaults = variantDefaults(set)
  const scored = set.variants.map((variant) => ({
    name: variant.name,
    score: Object.keys(defaults).filter(
      (name) => (variant.props[name] ?? defaults[name]) === requested[name]
    ).length
  }))
  return scored
    .sort((a, b) => b.score - a.score)
    .slice(0, 3)
    .map((entry) => entry.name)
}

// Only own keys count: a Figma property or variant value named like an Object member ("toString",
// "__proto__") must not match an inherited member of a parsed record.
const own = <T>(record: Record<string, T>, key: string): T | undefined =>
  Object.hasOwn(record, key) ? record[key] : undefined

function identify(
  ctx: ResolveContext,
  input: ResolveInput,
  captureRevision: string | undefined
): { errors: Diagnostic[]; set?: FigmaSet; setId?: string; variantId?: string } {
  if (!input.nodeId && !input.name) {
    return {
      errors: [
        diagnostic('invalid-input', 'Provide a Figma nodeId (preferred) or a component set name.')
      ]
    }
  }
  if (input.nodeId) {
    const id = normalizeNodeId(input.nodeId)
    const set = ctx.figmaSets.find(
      (candidate) =>
        candidate.setId === id || candidate.variants.some((variant) => variant.id === id)
    )
    if (set) {
      const variant = set.variants.find((candidate) => candidate.id === id)
      return {
        errors: [],
        set,
        setId: set.setId,
        variantId: variant && set.setId !== id ? id : undefined
      }
    }
    // Without a capture the id can only be matched against mappings.
    return { errors: [], setId: id }
  }
  const matches = ctx.figmaSets.filter((candidate) => candidate.name === input.name)
  if (matches.length > 1) {
    return {
      errors: [
        diagnostic(
          'identity-ambiguous',
          `"${input.name}" names ${matches.length} Figma components. Pass the nodeId instead.`,
          {
            reason: 'name',
            candidates: matches.map((m) => ({ nodeId: m.setId, name: m.name, page: m.page }))
          }
        )
      ]
    }
  }
  if (matches.length === 0) {
    return {
      errors: [
        ctx.figmaSets.length === 0
          ? diagnostic(
              'identity-uncertain',
              `No Figma capture is available to look up "${input.name}" by name. Pass the nodeId.`,
              {
                reason: 'name',
                name: input.name,
                ...(captureRevision ? { captureRevision } : {})
              }
            )
          : diagnostic(
              'identity-not-found',
              `No captured Figma component is named "${input.name}".`,
              { reason: 'name', name: input.name }
            )
      ]
    }
  }
  return { errors: [], set: matches[0], setId: matches[0]!.setId }
}

function renderUsage(
  mapping: ParsedMapping,
  selection: Record<string, string>,
  input: ResolveInput
): { usage?: string; errors: Diagnostic[] } {
  const errors: Diagnostic[] = []
  const values = new Map<string, string>()
  for (const local of mapping.locals) {
    if (local.kind === 'enum') {
      const picked = own(selection, local.figmaProperty)
      values.set(local.local, picked !== undefined ? (own(local.values, picked) ?? '') : '')
    } else if (local.kind === 'boolean') {
      const picked = own(selection, local.figmaProperty)?.toLowerCase()
      values.set(
        local.local,
        ['true', 'yes', 'on'].includes(picked ?? '') ? local.whenTrue : local.whenFalse
      )
    } else if (local.kind === 'string') {
      values.set(
        local.local,
        (input.texts && own(input.texts, local.figmaProperty)) ?? local.figmaProperty
      )
    } else {
      values.set(local.local, (input.texts && own(input.texts, local.layer)) ?? local.layer)
    }
  }
  const usage = mapping.example!.replace(/\$\{([^}]+)\}/g, (_, expression: string) => {
    const value = values.get(expression.trim())
    if (value === undefined) {
      errors.push(
        diagnostic(
          'unsupported-construct',
          `Example uses \${${expression}} which is not a declared local`
        )
      )
      return ''
    }
    const local = mapping.locals.find((item) => item.local === expression.trim())
    return local && (local.kind === 'string' || local.kind === 'text')
      ? value.replace(
          /[&<>"'{}\r\n]/g,
          (character) =>
            ({
              '&': '&amp;',
              '<': '&lt;',
              '>': '&gt;',
              '"': '&quot;',
              "'": '&#39;',
              '{': '&#123;',
              '}': '&#125;',
              '\r': '&#13;',
              '\n': '&#10;'
            })[character]!
        )
      : value
  })
  return { usage: errors.length ? undefined : usage, errors }
}

function translate(mapping: ParsedMapping, selection: Record<string, string>): PropTranslation[] {
  return mapping.locals.map((local): PropTranslation => {
    const codeProp = codePropOf(local, mapping.example ?? '')
    if (local.kind === 'enum') {
      const picked = own(selection, local.figmaProperty)
      return {
        figmaProperty: local.figmaProperty,
        figmaType: 'VARIANT',
        codeProp,
        values: Object.entries(local.values).map(([figma, code]) => ({ figma, code: code.trim() })),
        selected:
          picked !== undefined && own(local.values, picked) !== undefined
            ? { figma: picked, code: own(local.values, picked)!.trim() }
            : undefined,
        note: codeProp ? undefined : 'No prop name could be read from the example'
      }
    }
    if (local.kind === 'boolean') {
      return { figmaProperty: local.figmaProperty, figmaType: 'BOOLEAN', codeProp }
    }
    if (local.kind === 'string') {
      return {
        figmaProperty: local.figmaProperty,
        figmaType: 'TEXT',
        note: 'Rendered as children or text'
      }
    }
    return { figmaProperty: local.layer, figmaType: 'TEXT', note: `Text layer "${local.layer}"` }
  })
}

async function loadMappings(library: LibraryInfo, mappingRoot: string) {
  const files = await findMappingFiles(mappingRoot)
  return Promise.all(files.map((file) => readMappingFile(file)))
}

async function resolveComponent(ctx: ResolveContext, input: ResolveInput): Promise<ResolveResult> {
  if (!ctx.mappingRoot) {
    return fail([
      diagnostic(
        'mapping-root-not-configured',
        'No mapping source configured. Set --mapping-root, DCL_FIGMA_MAPPING_ROOT or contract.mappingRoot in figma.config.json.'
      )
    ])
  }
  const library = await loadLibrary(ctx.mappingRoot)
  if (!library) {
    return fail([
      diagnostic(
        'mapping-root-not-configured',
        `${ctx.mappingRoot} is not inside an npm package, so the library version and name cannot be established.`
      )
    ])
  }

  const warnings: Diagnostic[] = []
  if (ctx.figmaSets.length === 0) {
    warnings.push(
      diagnostic(
        'figma-identity-unverified',
        'No Figma capture available: component identity and variants are checked against the mapping only.'
      )
    )
  }

  // The revision is provenance only: an unreadable lock is reported, never a reason to refuse the resolve.
  let captureRevision: string | undefined
  try {
    captureRevision = await readCaptureRevision(library.rootDir)
  } catch (error) {
    warnings.push(
      diagnostic(
        'capture-revision-unknown',
        `The library lock pins no readable capture revision: ${(error as Error).message}`
      )
    )
  }

  const identity = identify(ctx, input, captureRevision)
  if (identity.errors.length) return fail(identity.errors, warnings)

  const mappings = await loadMappings(library, ctx.mappingRoot)
  const candidates = mappings.filter((mapping) => mapping.figma?.nodeId === identity.setId)
  if (candidates.length === 0) {
    // mapping-missing only once identity is established: the node is a captured library component.
    if (!identity.set) {
      return fail(
        [
          ctx.figmaSets.length === 0
            ? diagnostic(
                'identity-uncertain',
                `No Figma capture is available and no mapping names node ${identity.setId}, so it cannot be established whether it is a library component.`,
                {
                  reason: 'node',
                  nodeId: identity.setId,
                  ...(captureRevision ? { captureRevision } : {})
                }
              )
            : diagnostic(
                'identity-not-found',
                `Figma node ${identity.setId} is not a component of the captured library.`,
                { reason: 'node', nodeId: identity.setId }
              )
        ],
        warnings
      )
    }
    const set = identity.set
    const sameName = mappings.filter(
      (m) => m.header.component?.toLowerCase() === set.name.toLowerCase()
    )
    return fail(
      [
        diagnostic(
          'mapping-missing',
          `No Figma-to-React mapping for "${set.name}" (${identity.setId}). The resolver does not guess or generate a replacement.`,
          {
            nodeId: identity.setId,
            name: set.name,
            variantProperties: input.variantProperties ?? {},
            library: { name: library.name, version: library.version },
            ...(captureRevision ? { captureRevision } : {}),
            supported: [
              ...new Set(mappings.flatMap((m) => (m.header.component ? [m.header.component] : [])))
            ].sort(),
            sameNameMappings: sameName.map((m) => ({
              file: path.relative(library.rootDir, m.file),
              targets: m.figma?.nodeId
            }))
          }
        )
      ],
      warnings
    )
  }
  if (candidates.length > 1) {
    return fail(
      [
        diagnostic(
          'identity-ambiguous',
          `${candidates.length} mappings claim Figma node ${identity.setId}.`,
          {
            reason: 'mappings',
            nodeId: identity.setId,
            files: candidates.map((m) => path.relative(library.rootDir, m.file)),
            ...(captureRevision ? { captureRevision } : {})
          }
        )
      ],
      warnings
    )
  }

  const mapping = candidates[0]!
  const errors = await validateAgainstLibrary(mapping, {
    library,
    libraryRange: ctx.libraryRange
  })
  if (identity.set) errors.push(...validateAgainstFigma(mapping, identity.set))
  const blocking = errors.filter((d) => d.severity === 'error')
  warnings.push(...errors.filter((d) => d.severity === 'warning'))
  if (blocking.length) return fail(blocking, warnings)

  // Variant selection: the variant node, then explicit properties, over the Figma defaults.
  const set = identity.set
  let selection: Record<string, string> = {}
  let basis: 'requested' | 'figma-defaults' = 'figma-defaults'
  if (set) {
    selection = variantDefaults(set)
    const variant = set.variants.find((candidate) => candidate.id === identity.variantId)
    if (variant) {
      selection = { ...selection, ...variant.props }
      basis = 'requested'
    }
  }
  if (input.variantProperties && Object.keys(input.variantProperties).length > 0) {
    selection = { ...selection, ...input.variantProperties }
    basis = 'requested'
  }

  if (set) {
    for (const [name, value] of Object.entries(selection)) {
      const property = own(set.properties, name)
      if (!property || (property.options && !property.options.includes(value))) {
        return fail(
          [
            diagnostic(
              'variant-not-in-figma',
              property
                ? `"${value}" is not a value of Figma property "${name}"`
                : `"${set.name}" has no Figma property "${name}"`,
              {
                property: name,
                value,
                options: property?.options,
                properties: Object.keys(set.properties)
              }
            )
          ],
          warnings
        )
      }
    }
    if (!comboExists(set, selection)) {
      return fail(
        [
          diagnostic(
            'variant-not-in-figma',
            `Figma "${set.name}" has no variant for ${JSON.stringify(selection)}. The design does not define this combination.`,
            { requested: selection, nearest: nearestVariants(set, selection) }
          )
        ],
        warnings
      )
    }
  }

  for (const local of mapping.locals) {
    if (local.kind !== 'enum') continue
    const picked = own(selection, local.figmaProperty)
    if (picked !== undefined && own(local.values, picked) === undefined) {
      return fail(
        [
          diagnostic(
            'variant-unsupported',
            `The mapping does not support ${local.figmaProperty}="${picked}". Supported: ${Object.keys(local.values).join(', ')}.`,
            { property: local.figmaProperty, value: picked, supported: Object.keys(local.values) }
          )
        ],
        warnings
      )
    }
  }

  const rendered = renderUsage(mapping, selection, input)
  if (rendered.errors.length) return fail(rendered.errors, warnings)

  const specifier = importSpecifier(mapping)!
  return {
    ok: true,
    warnings,
    resolved: {
      component: {
        exportName: mapping.header.component!,
        importSpecifier: specifier,
        importStatements: mapping.imports
      },
      usage: rendered.usage!,
      usageFor: { variantProperties: selection, basis },
      propTranslation: translate(mapping, selection),
      interactionStates: mapping.header.states,
      provenance: {
        figma: {
          fileKey: mapping.figma!.fileKey,
          setId: mapping.figma!.nodeId,
          setName: set?.name,
          page: set?.page,
          variantNodeId: identity.variantId,
          url: mapping.header.url!,
          identity: set ? 'verified-against-capture' : 'mapping-only'
        },
        mapping: {
          file: path.relative(library.rootDir, mapping.file),
          id: mapping.id,
          contract: mapping.header.contract!
        },
        library: { name: library.name, version: library.version },
        ...(captureRevision ? { capture: { revision: captureRevision } } : {})
      }
    }
  }
}

// Used by CI: every mapping must validate against the library and, when a capture exists, the design.
async function checkMappings(
  ctx: ResolveContext
): Promise<{ file: string; diagnostics: Diagnostic[] }[]> {
  if (!ctx.mappingRoot) return []
  const library = await loadLibrary(ctx.mappingRoot)
  if (!library) return []
  const mappings = await loadMappings(library, ctx.mappingRoot)
  const results: { file: string; diagnostics: Diagnostic[] }[] = []
  const seenNodes = new Map<string, string>()
  const seenIds = new Map<string, string>()
  const synced = (await ctx.loadSyncedComponents?.()) ?? new Map<string, string[]>()
  for (const mapping of mappings) {
    const rel = path.relative(library.rootDir, mapping.file)
    const blockers = mapping.header.component ? synced.get(mapping.header.component) : undefined
    if (blockers) {
      results.push({
        file: rel,
        diagnostics: [
          blockers.length
            ? diagnostic(
                'superseded-pending',
                `${rel}: hand-written ${mapping.header.component} stays until its synced replacement can be promoted: ${blockers.join('; ')}`,
                { reasons: blockers }
              )
            : diagnostic(
                'superseded-by-sync',
                `${rel}: hand-written ${mapping.header.component} is replaced by the synced component promotion.include names on the next promotion`
              )
        ]
      })
      continue
    }
    const found = await validateAgainstLibrary(mapping, {
      library,
      libraryRange: ctx.libraryRange
    })
    if (mapping.figma) {
      const set = ctx.figmaSets.find((candidate) => candidate.setId === mapping.figma!.nodeId)
      if (ctx.figmaSets.length > 0 && !set) {
        found.push(
          diagnostic(
            'identity-not-found',
            `${rel}: Figma node ${mapping.figma.nodeId} is not in the capture`,
            { reason: 'mapping-node', file: rel, nodeId: mapping.figma.nodeId }
          )
        )
      }
      if (set) found.push(...validateAgainstFigma(mapping, set))
      const previous = seenNodes.get(mapping.figma.nodeId)
      if (previous) {
        found.push(
          diagnostic(
            'identity-ambiguous',
            `${rel} and ${previous} map the same Figma node ${mapping.figma.nodeId}`,
            { reason: 'duplicate-node', files: [previous, rel], nodeId: mapping.figma.nodeId }
          )
        )
      }
      seenNodes.set(mapping.figma.nodeId, rel)
    }
    if (mapping.id) {
      const previous = seenIds.get(mapping.id)
      if (previous) {
        found.push(
          diagnostic(
            'identity-ambiguous',
            `${rel} and ${previous} share the template id "${mapping.id}"`,
            { reason: 'duplicate-id', files: [previous, rel], id: mapping.id }
          )
        )
      }
      seenIds.set(mapping.id, rel)
    }
    results.push({ file: rel, diagnostics: found })
  }
  return results
}

export { checkMappings, resolveComponent }
export type { ResolveContext, ResolveInput, ResolveResult, Resolved }
