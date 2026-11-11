import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'
import { componentRegistryEntries, type AcceptedLimit } from '../../component-generation.mjs'
import { diagnostic, type Diagnostic } from './diagnostics.js'
import { normalizeNodeId, type FigmaSet } from './figma-index.js'
import { findMappingFiles, loadLibrary, readCaptureRevision, type LibraryInfo } from './library.js'
import { readMappingFile, type MappingLocal, type ParsedMapping } from './mapping-file.js'
import { importSpecifier, validateAgainstFigma, validateAgainstLibrary } from './validate.js'
import { resolveByName, setsFittingVariantProperties } from './name-identity.js'

type ResolveContext = {
  mappingRoot?: string
  libraryRange?: string
  // Empty when no capture is available; identity is then checked against the mapping only.
  figmaSets: FigmaSet[]
  // Synced components promotion.include names and promotion has not written yet, with the reasons
  // promotion would skip each one (none when eligible). Any other hand-written mapping is validated.
  loadSyncedComponents?: () => Promise<Map<string, string[]>>
  // figma.config.json componentNames: the code name of a Figma set name, for name lookups without a capture.
  componentNames?: Readonly<Record<string, string>>
}

type ResolveInput = {
  nodeId?: string
  name?: string
  variantProperties?: Record<string, string>
  // Figma BOOLEAN component properties, "true" or "false": kept apart from VARIANT properties because a
  // boolean the mapping does not read is only a warning, while an unread variant is a misfit.
  booleanProperties?: Record<string, string>
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
  // `unspecified`: no capture and no variant or boolean properties, so the usage shows the mapping's empty
  // selection.
  usageFor: {
    variantProperties: Record<string, string>
    booleanProperties: Record<string, string>
    basis: 'requested' | 'figma-defaults' | 'unspecified'
  }
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
  // Gaps the library owners accepted for this component (components/promotion.json), repeated verbatim.
  limits: AcceptedLimit[]
}

// What a contributor report needs from a failure: the library and the capture it was checked against.
type FailureContext = { library: { name: string; version: string }; capture?: { revision: string } }

type ResolveResult =
  | { ok: true; resolved: Resolved; warnings: Diagnostic[] }
  | { ok: false; errors: Diagnostic[]; warnings: Diagnostic[]; context?: FailureContext }

const fail = (
  errors: Diagnostic[],
  warnings: Diagnostic[] = [],
  context?: FailureContext
): ResolveResult =>
  context ? { ok: false, errors, warnings, context } : { ok: false, errors, warnings }

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

const TRUE_WORDS = ['true', 'yes', 'on']
const FALSE_WORDS = ['false', 'no', 'off']

// Only own keys count: a Figma property or variant value named like an Object member ("toString",
// "__proto__") must not match an inherited member of a parsed record.
const own = <T>(record: Record<string, T>, key: string): T | undefined =>
  Object.hasOwn(record, key) ? record[key] : undefined

function identify(
  ctx: ResolveContext,
  input: ResolveInput
): {
  errors: Diagnostic[]
  set?: FigmaSet
  setId?: string
  variantId?: string
  // A name to resolve against the mappings, when no capture can identify it.
  byName?: string
} {
  if (!input.nodeId && !input.name) {
    return {
      errors: [
        diagnostic('invalid-input', 'Provide a Figma nodeId (preferred) or a component set name.')
      ]
    }
  }
  const notBoolean = Object.entries(input.booleanProperties ?? {}).find(
    ([, value]) => value !== 'true' && value !== 'false'
  )
  if (notBoolean)
    return {
      errors: [
        diagnostic(
          'invalid-input',
          `The boolean property "${notBoolean[0]}" must be "true" or "false", as Figma reports it.`,
          { property: notBoolean[0], value: notBoolean[1] }
        )
      ]
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
  if (ctx.figmaSets.length === 0) return { errors: [], byName: input.name }
  // A set captured on two pages is one component; only distinct sets make a name ambiguous.
  const matches = [
    ...new Map(
      ctx.figmaSets
        .filter((candidate) => candidate.name === input.name)
        .map((candidate) => [candidate.setId, candidate])
    ).values()
  ]
  const fitting = setsFittingVariantProperties(matches, input.variantProperties)
  if (fitting.length === 1) return { errors: [], set: fitting[0], setId: fitting[0]!.setId }
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
        diagnostic('identity-not-found', `No captured Figma component is named "${input.name}".`, {
          reason: 'name',
          name: input.name
        })
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
      const picked = booleanPick(local, selection, input.booleanProperties)?.toLowerCase()
      values.set(local.local, TRUE_WORDS.includes(picked ?? '') ? local.whenTrue : local.whenFalse)
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

// The value a boolean local renders: the variant selection first, then the instance's BOOLEAN properties, as
// renderUsage reads it.
function booleanPick(
  local: { figmaProperty: string },
  selection: Record<string, string>,
  booleans: Readonly<Record<string, string>> | undefined
): string | undefined {
  return own(selection, local.figmaProperty) ?? (booleans && own(booleans, local.figmaProperty))
}

function translate(
  mapping: ParsedMapping,
  selection: Record<string, string>,
  booleans?: Readonly<Record<string, string>>
): PropTranslation[] {
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
      const picked = booleanPick(local, selection, booleans)
      return {
        figmaProperty: local.figmaProperty,
        figmaType: 'BOOLEAN',
        codeProp,
        ...(picked === undefined
          ? {}
          : {
              selected: {
                figma: picked,
                code: (TRUE_WORDS.includes(picked.toLowerCase())
                  ? local.whenTrue
                  : local.whenFalse
                ).trim()
              }
            })
      }
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

// Everything a resolve reads from the library, loaded once per command: a batch shares it across its entries.
type LibrarySource = {
  library: LibraryInfo
  mappingRoot: string
  mappings: ParsedMapping[]
  // Accepted limits by public component name, or the coded failure of an unreadable registry.
  limits: Readonly<Record<string, AcceptedLimit[]>> | { error: Diagnostic }
  captureRevision?: string
  captureWarning?: Diagnostic
}

const notConfigured = (message: string, details?: Record<string, unknown>) => ({
  error: diagnostic('mapping-root-not-configured', message, details)
})

const errorCode = (error: unknown) => (error as NodeJS.ErrnoException).code

// The mapping root as a coded answer: unset, not an existing directory, unreadable, outside an npm package or in a
// package without a readable name and version are each mapping-root-not-configured, never a raw error.
async function mappingSource(
  mappingRoot: string | undefined
): Promise<{ library: LibraryInfo; mappingRoot: string } | { error: Diagnostic }> {
  if (!mappingRoot)
    return notConfigured(
      'No mapping source configured. Set --mapping-root, DCL_FIGMA_MAPPING_ROOT or contract.mappingRoot in figma.config.json.'
    )
  const shown = path.resolve(mappingRoot)
  let directory
  try {
    directory = await stat(mappingRoot)
  } catch (error) {
    const code = errorCode(error)
    return code === 'ENOENT' || code === 'ENOTDIR'
      ? notConfigured(`${shown} is not an existing directory.`)
      : notConfigured(`${shown} cannot be read (${code ?? 'error'}).`, { code })
  }
  if (!directory.isDirectory()) return notConfigured(`${shown} is not an existing directory.`)
  const library = await loadLibrary(mappingRoot)
  if (!library)
    return notConfigured(
      `${shown} is not inside an npm package, so the library version and name cannot be established.`
    )
  if ('invalid' in library)
    return notConfigured(
      `${library.invalid}, so the library version and name cannot be established.`
    )
  return { library, mappingRoot }
}

// The registry (components/promotion.json) of the library that owns the mappings: the nearest one from the
// mapping root up to the package root, so a root pointed at a subdirectory still finds it. A library without a
// registry has no accepted limits; a registry that cannot be read or is not valid is a coded failure.
async function registryLimits(
  library: LibraryInfo,
  mappingRoot: string
): Promise<LibrarySource['limits']> {
  const top = path.resolve(library.rootDir)
  for (let dir = path.resolve(mappingRoot); ; dir = path.dirname(dir)) {
    for (const file of [
      path.join(dir, 'promotion.json'),
      path.join(dir, 'components/promotion.json')
    ]) {
      let text: string
      try {
        text = await readFile(file, 'utf8')
      } catch (error) {
        if (errorCode(error) === 'ENOENT' || errorCode(error) === 'ENOTDIR') continue
        return registryError(file, `cannot be read (${errorCode(error) ?? 'error'})`)
      }
      try {
        return componentRegistryEntries(JSON.parse(text)).limits
      } catch (error) {
        return registryError(file, (error as Error).message)
      }
    }
    if (dir === top || path.dirname(dir) === dir) return {}
  }
}

const registryError = (file: string, problem: string) => ({
  error: diagnostic(
    'contract-incompatible',
    `The library component registry ${file} is not valid: ${problem}.`,
    { reason: 'component-registry' }
  )
})

async function loadSource(ctx: ResolveContext): Promise<LibrarySource | { error: Diagnostic }> {
  const root = await mappingSource(ctx.mappingRoot)
  if ('error' in root) return root
  const { library, mappingRoot } = root
  let mappings: ParsedMapping[]
  try {
    const files = await findMappingFiles(mappingRoot)
    mappings = await Promise.all(files.map((file) => readMappingFile(file)))
  } catch (error) {
    const code = errorCode(error)
    if (!code) throw error
    // A subdirectory or mapping file below the root that cannot be read (EACCES, a dangling symlink).
    const where = (error as NodeJS.ErrnoException).path ?? path.resolve(mappingRoot)
    return notConfigured(`${where} under the mapping root cannot be read (${code}).`, { code })
  }
  // The revision is provenance only: an unreadable lock is reported, never a reason to refuse the resolve.
  let captureRevision: string | undefined
  let captureWarning: Diagnostic | undefined
  try {
    captureRevision = await readCaptureRevision(library.rootDir)
  } catch (error) {
    captureWarning = diagnostic(
      'capture-revision-unknown',
      `The library lock pins no readable capture revision: ${(error as Error).message}`,
      { lock: `${library.name}/upstream.lock.json` }
    )
  }
  return {
    library,
    mappingRoot,
    mappings,
    limits: await registryLimits(library, mappingRoot),
    captureRevision,
    captureWarning
  }
}

// `source` lets a batch load the library once for all its entries; a single resolve loads it here.
async function resolveComponent(
  ctx: ResolveContext,
  input: ResolveInput,
  source?: LibrarySource | { error: Diagnostic }
): Promise<ResolveResult> {
  const loaded = source ?? (await loadSource(ctx))
  if ('error' in loaded) return fail([loaded.error])
  const { library, mappings, captureRevision } = loaded

  const warnings: Diagnostic[] = []
  if (ctx.figmaSets.length === 0) {
    warnings.push(
      diagnostic(
        'figma-identity-unverified',
        'No Figma capture available: component identity and variants are checked against the mapping only.'
      )
    )
  }

  if (loaded.captureWarning) warnings.push(loaded.captureWarning)
  const context: FailureContext = {
    library: { name: library.name, version: library.version },
    ...(captureRevision ? { capture: { revision: captureRevision } } : {})
  }
  const failure = (errors: Diagnostic[]) => fail(errors, warnings, context)

  const identity = identify(ctx, input)
  if (identity.errors.length) return failure(identity.errors)

  // With a capture, a node outside it is not a library component, even if a stale mapping still targets it.
  if (ctx.figmaSets.length > 0 && !identity.set) {
    // A mapping that still targets the node is a stale mapping: a library defect to report, not a retry.
    const stale = mappings.filter((mapping) => mapping.figma?.nodeId === identity.setId)
    return failure([
      stale.length
        ? diagnostic(
            'identity-not-found',
            `Figma node ${identity.setId} is not in the capture, but a mapping still targets it.`,
            {
              reason: 'stale-mapping',
              nodeId: identity.setId,
              files: stale.map((mapping) => path.relative(library.rootDir, mapping.file))
            }
          )
        : diagnostic(
            'identity-not-found',
            `Figma node ${identity.setId} is not a component of the captured library.`,
            { reason: 'node', nodeId: identity.setId }
          )
    ])
  }
  if (identity.byName !== undefined) {
    const named = resolveByName(
      mappings,
      identity.byName,
      input,
      ctx.componentNames,
      library,
      captureRevision ? { captureRevision } : {}
    )
    if ('errors' in named) return failure(named.errors)
    identity.setId = named.setId
    warnings.push(named.warning)
  } else if (input.name !== undefined && identity.set && !input.nodeId)
    warnings.push(
      diagnostic('identity-name-match', `Identified "${input.name}" by its captured set name.`, {
        name: input.name,
        matchedBy: 'capture'
      })
    )
  const candidates = mappings.filter((mapping) => mapping.figma?.nodeId === identity.setId)
  if (candidates.length === 0) {
    // mapping-missing only once identity is established: the node is a captured library component.
    // Only reachable without a capture: a node outside a loaded capture was refused above.
    if (!identity.set) {
      return failure([
        diagnostic(
          'identity-uncertain',
          `No Figma capture is available and no mapping names node ${identity.setId}, so it cannot be established whether it is a library component.`,
          {
            reason: 'node',
            nodeId: identity.setId,
            ...(captureRevision ? { captureRevision } : {})
          }
        )
      ])
    }
    const set = identity.set
    const sameName = mappings.filter(
      (m) => m.header.component?.toLowerCase() === set.name.toLowerCase()
    )
    return failure([
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
    ])
  }
  if (candidates.length > 1) {
    return failure([
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
    ])
  }

  const mapping = candidates[0]!
  const errors = await validateAgainstLibrary(mapping, {
    library,
    libraryRange: ctx.libraryRange
  })
  if (identity.set) errors.push(...validateAgainstFigma(mapping, identity.set))
  const blocking = errors.filter((d) => d.severity === 'error')
  warnings.push(...errors.filter((d) => d.severity === 'warning'))
  if (blocking.length) return failure(blocking)

  // Variant selection: the variant node, then explicit properties, over the Figma defaults.
  const set = identity.set
  let selection: Record<string, string> = {}
  let basis: 'requested' | 'figma-defaults' | 'unspecified' = set ? 'figma-defaults' : 'unspecified'
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
        return failure([
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
        ])
      }
    }
    if (!comboExists(set, selection)) {
      return failure([
        diagnostic(
          'variant-not-in-figma',
          `Figma "${set.name}" has no variant for ${JSON.stringify(selection)}. The design does not define this combination.`,
          { requested: selection, nearest: nearestVariants(set, selection) }
        )
      ])
    }
  }

  // A boolean the mapping reads takes only the accepted words, with or without a capture. Without a capture, a
  // property the mapping does not read is reported the way the capture path reports one: a warning, not a refusal.
  const reads = new Map(
    mapping.locals.flatMap((local) =>
      local.kind === 'enum' || local.kind === 'boolean'
        ? [[local.figmaProperty, local.kind] as const]
        : []
    )
  )
  for (const [property, value] of Object.entries(input.variantProperties ?? {})) {
    const kind = reads.get(property)
    if (kind === 'boolean' && ![...TRUE_WORDS, ...FALSE_WORDS].includes(value.toLowerCase()))
      return failure([
        diagnostic('variant-unsupported', `"${value}" is not a boolean value for "${property}".`, {
          property,
          value,
          supported: [...TRUE_WORDS, ...FALSE_WORDS]
        })
      ])
    if (!set && kind === undefined && !mapping.header.states.includes(property))
      warnings.push(
        diagnostic(
          'unmapped-figma-property',
          `The mapping does not read the property "${property}".`,
          {
            property,
            supported: [...reads.keys(), ...mapping.header.states]
          }
        )
      )
  }
  for (const [property, value] of Object.entries(input.booleanProperties ?? {})) {
    const kind = reads.get(property)
    if (kind === 'enum')
      return failure([
        diagnostic(
          'variant-unsupported',
          `"${property}" is a boolean in the design, but the mapping reads it as a variant.`,
          { property, value, supported: [...reads.keys()] }
        )
      ])
    if (kind === undefined && !mapping.header.states.includes(property))
      warnings.push(
        diagnostic(
          'unmapped-figma-property',
          `The mapping does not read the boolean property "${property}".`,
          { property, supported: [...reads.keys(), ...mapping.header.states] }
        )
      )
  }
  const textKeys = new Set(
    mapping.locals.flatMap((local) =>
      local.kind === 'string' ? [local.figmaProperty] : local.kind === 'text' ? [local.layer] : []
    )
  )
  for (const key of Object.keys(input.texts ?? {}))
    if (!textKeys.has(key))
      warnings.push(
        diagnostic('unmapped-figma-property', `The mapping renders no text for "${key}".`, {
          text: key,
          texts: [...textKeys]
        })
      )

  for (const local of mapping.locals) {
    if (local.kind !== 'enum') continue
    const picked = own(selection, local.figmaProperty)
    if (picked !== undefined && own(local.values, picked) === undefined) {
      return failure([
        diagnostic(
          'variant-unsupported',
          `The mapping does not support ${local.figmaProperty}="${picked}". Supported: ${Object.keys(local.values).join(', ')}.`,
          { property: local.figmaProperty, value: picked, supported: Object.keys(local.values) }
        )
      ])
    }
  }

  const rendered = renderUsage(mapping, selection, input)
  if (rendered.errors.length) return failure(rendered.errors)

  // Without a capture nothing checks that the variant values form a combination the design defines, and a mapped
  // variant property the instance did not give is left out of the usage: the warning names both.
  if (!set) {
    const notGiven = mapping.locals.flatMap((local) =>
      local.kind === 'enum' && own(selection, local.figmaProperty) === undefined
        ? [local.figmaProperty]
        : []
    )
    const index = warnings.findIndex((warning) => warning.code === 'figma-identity-unverified')
    if (index >= 0)
      warnings[index] = {
        ...warnings[index]!,
        message: `${warnings[index]!.message} The combination of variant values is not checked against the design${notGiven.length ? `, and ${notGiven.join(', ')} ${notGiven.length === 1 ? 'was' : 'were'} not given, so the usage leaves ${notGiven.length === 1 ? 'it' : 'them'} out` : ''}.`,
        details: { combinationChecked: false, notGiven }
      }
  }

  // An unreadable registry fails only a resolve that reaches a component, so identity failures keep their codes.
  if ('error' in loaded.limits) return failure([loaded.limits.error as Diagnostic])
  const limits = Object.hasOwn(loaded.limits, mapping.header.component!)
    ? (loaded.limits as Record<string, AcceptedLimit[]>)[mapping.header.component!]!
    : []
  if (limits.length)
    warnings.push(
      diagnostic(
        'accepted-limits',
        `${mapping.header.component} has ${limits.length} accepted ${limits.length === 1 ? 'limit' : 'limits'}.`,
        { ids: limits.map((limit) => limit.id) }
      )
    )

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
      usageFor: {
        variantProperties: selection,
        booleanProperties: { ...input.booleanProperties },
        basis:
          basis === 'unspecified' && Object.keys(input.booleanProperties ?? {}).length > 0
            ? 'requested'
            : basis
      },
      propTranslation: translate(mapping, selection, input.booleanProperties),
      interactionStates: mapping.header.states,
      limits,
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
  // checkMappingsReport turns an unusable root into mapping-root-not-configured; callers of this list get none.
  const source = await loadSource(ctx)
  return 'error' in source ? [] : checkLoadedMappings(ctx, source)
}

async function checkLoadedMappings(
  ctx: ResolveContext,
  { library, mappings }: LibrarySource
): Promise<{ file: string; diagnostics: Diagnostic[] }[]> {
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

// The check-mappings answer for the CLI and the MCP tool. Without a capture the node, drift and
// supersession checks cannot run, and the report says so instead of passing silently.
async function checkMappingsReport(ctx: ResolveContext) {
  // A missing, absent or unpackaged mapping root is a failure here too, never an empty pass. The library is
  // loaded once, so a root that breaks mid-check cannot turn into an empty pass either.
  const source = await loadSource(ctx)
  if ('error' in source)
    return {
      ok: false,
      checked: 0,
      captureChecked: false,
      errors: [source.error],
      warnings: [],
      results: []
    }
  const results = await checkLoadedMappings(ctx, source)
  const failing = results.filter((r) => r.diagnostics.some((d) => d.severity === 'error'))
  const captureChecked = ctx.figmaSets.length > 0
  return {
    ok: failing.length === 0,
    checked: results.length,
    captureChecked,
    warnings: captureChecked
      ? []
      : [
          diagnostic(
            'capture-unchecked',
            'No Figma capture: mappings were checked against the library only; the capture node, drift and supersession checks did not run.'
          )
        ],
    results
  }
}

export { checkMappings, checkMappingsReport, loadSource, resolveComponent }
export type { FailureContext, LibrarySource, ResolveContext, ResolveInput, ResolveResult, Resolved }
