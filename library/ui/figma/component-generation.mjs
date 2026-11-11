import { Buffer } from 'node:buffer'
import { createHash } from 'node:crypto'
import { moduleClosure } from './module-graph.mjs'
import { componentSourceIdentity } from './source-identity.mjs'

const ROOT = 'library/ui/figma/'
const RUNTIME = [
  'figma-module-runtime.tsx',
  'figma-module-runtime.module.css',
  'figma-view-state.ts',
  'figma-interactions.tsx'
]
// Always bound. The code among them roots the import closure; the runtime sources are copied as bytes.
const MODULES = [
  ROOT + 'component-generation.mjs',
  ROOT + 'module-graph.mjs',
  ROOT + 'source-identity.mjs',
  ROOT + 'ledger/retirements.json',
  ROOT + 'tsconfig.json',
  ROOT + 'src/install-main.ts',
  ROOT + 'src/install.ts',
  ROOT + 'src/install-components.ts',
  ROOT + 'src/generators/components.ts',
  ROOT + 'src/generators/tokens.ts',
  ...RUNTIME.map((file) => ROOT + 'review/src/components/' + file)
]
const CODE = /\.(?:[cm]?[jt]sx?)$/i
const RUNTIME_PATHS = RUNTIME.map((file) => ROOT + 'review/src/components/' + file)
const ROOTS = MODULES.filter((file) => CODE.test(file) && !RUNTIME_PATHS.includes(file))
/** The only packages a generation module may import; their installed contents are bound by the dependency manifests. */
export const componentPackages = [
  'postcss',
  'postcss-modules',
  'svgo',
  'zod',
  'semver',
  'typescript'
]
export const componentDependencyPaths = [
  'library/ui/package.json',
  'library/ui/pnpm-lock.yaml',
  'library/ui/tsconfig.json'
]
export const componentGenerationFile = 'generation.json'
export const componentConfigPath = ROOT + 'figma.config.json'
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex')
const digest = (value) => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value)
const blob = (value) => typeof value === 'string' && /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(value)
const record = (value) => value !== null && typeof value === 'object' && !Array.isArray(value)
const safe = (value) =>
  typeof value === 'string' &&
  !/[\\<>:"|?*]/.test(value) &&
  ![...value].some(
    (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
  ) &&
  value
    .split('/')
    .every(
      (part) =>
        part &&
        !['.', '..', '__proto__', 'prototype', 'constructor'].includes(part) &&
        !/[. ]$/.test(part) &&
        !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)
    )
const ordered = (entries) => [...entries].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))

/**
 * Hand-written components: the top-level directories of library/ui/components that hold none of the installer's
 * outputs (`owned`, paths relative to library/ui/components, as the lock's public component table names them). Git
 * owns their files; the installer keeps them byte for byte and the lock never pins them.
 * @param {Iterable<string>} files paths relative to library/ui/components
 * @param {Iterable<string>} owned
 * @returns {string[]}
 */
export function handWrittenComponentDirectories(files, owned) {
  const top = (paths) =>
    new Set([...paths].filter((file) => file.includes('/')).map((file) => file.split('/')[0]))
  const installed = top(owned)
  return [...top(files)].filter((directory) => !installed.has(directory)).sort()
}

/**
 * Whether a tracked path can be a component generation input: a fixed module, a non-test file at the top of
 * library/ui/figma with a code extension, or any non-test file under library/ui/figma/src. The read-set is narrower
 * (componentModulePaths); this wider class is what the install and offline inventories compare with the source
 * lock, so no candidate can shadow a resolved import unpinned, and what port verification never exempts.
 */
export function isComponentModulePath(file) {
  return (
    typeof file === 'string' &&
    (MODULES.includes(file) ||
      (file.startsWith(ROOT) &&
        !file.slice(ROOT.length).includes('/') &&
        /\.(?:js|jsx|ts|tsx|mjs|mts|cjs|cts)$/i.test(file) &&
        !/\.(?:test|spec)\.[^.]+$/.test(file)) ||
      (file.startsWith(ROOT + 'src/') &&
        !/(?:\.(?:test|spec)\.[^.]+$|\/(?:fixtures|test|__tests__)\/)/.test(file)))
  )
}

/** The candidates a source lock pins, which the tracked and physical inventories must match exactly. */
export function componentModuleCandidates(source) {
  const files = source?.files
  if (!record(files)) throw new Error('No locked component module table')
  return Object.keys(files).filter(isComponentModulePath).sort()
}

// The scope the closure may resolve into: the top of library/ui/figma and its src tree, which the install and
// offline checks inventory for untracked shadows.
const inScope = (file) =>
  file.startsWith(ROOT) &&
  (!file.slice(ROOT.length).includes('/') || file.startsWith(ROOT + 'src/'))

// tsx resolves through library/ui/figma/tsconfig.json, so an alias there would load files the closure does not
// follow. The file is a fixed module; an alias, base URL or inherited configuration fails instead of being modeled.
function assertNoResolverAliases(text) {
  let parsed
  try {
    parsed = JSON.parse(text)
  } catch (error) {
    throw new Error('The component resolver configuration is not plain JSON', { cause: error })
  }
  const options = record(parsed) ? parsed.compilerOptions : undefined
  if (
    !record(parsed) ||
    'extends' in parsed ||
    (options !== undefined && !record(options)) ||
    ['paths', 'baseUrl', 'rootDirs'].some((key) => options && key in options)
  )
    throw new Error('The component resolver configuration adds module aliases')
}

/**
 * The generation read-set: the fixed modules, every module the installer entry points reach through static imports
 * (module-graph.mjs, fail closed) and every non-code file under library/ui/figma/src, which code may read at run
 * time. `read` returns a locked file's bytes; each read is checked against the lock, so the set is a function of
 * the lock alone. Tooling outside the closure is not a generation input.
 * @param {{files: Record<string, {sha256: string}>}} source
 * @param {(file: string) => Buffer | string} read
 * @returns {string[]}
 */
export function componentModulePaths(source, read) {
  const files = source?.files
  if (!record(files)) throw new Error('No locked component module table')
  if (typeof read !== 'function') throw new Error('Component module paths need a module reader')
  for (const file of MODULES)
    if (!Object.hasOwn(files, file))
      throw new Error('Missing locked component implementation: ' + file)
  const locked = (file) => {
    if (!safe(file)) throw new Error('Invalid locked component module: ' + file)
    const bytes = read(file)
    if (typeof bytes !== 'string' && !Buffer.isBuffer(bytes))
      throw new Error('Unreadable locked component module: ' + file)
    if (!digest(files[file]?.sha256) || hash(bytes) !== files[file].sha256)
      throw new Error('Component module differs from source lock: ' + file)
    return bytes.toString('utf8')
  }
  assertNoResolverAliases(locked(ROOT + 'tsconfig.json'))
  const closure = moduleClosure({
    roots: ROOTS,
    files: Object.keys(files),
    read: locked,
    scope: inScope,
    packages: componentPackages
  })
  const data = Object.keys(files).filter(
    (file) => file.startsWith(ROOT + 'src/') && isComponentModulePath(file) && !CODE.test(file)
  )
  const paths = [...new Set([...MODULES, ...closure, ...data])]
  for (const file of paths)
    if (!safe(file) || !digest(files[file]?.sha256))
      throw new Error('Invalid locked component module: ' + file)
  if (new Set(paths.map((file) => file.normalize('NFC').toLowerCase())).size !== paths.length)
    throw new Error('Colliding locked component module paths')
  return paths.sort()
}

function identities(values, expected, label) {
  if (!Array.isArray(values) || values.length !== expected.length)
    throw new Error('Incomplete component ' + label + ' identities')
  const found = new Map()
  for (const value of values) {
    if (
      !record(value) ||
      Object.keys(value).sort().join(',') !== 'indexBlobSha,path,sha256' ||
      !safe(value.path) ||
      !digest(value.sha256) ||
      !blob(value.indexBlobSha) ||
      found.has(value.path) ||
      !expected.includes(value.path)
    )
      throw new Error('Invalid component ' + label + ' identity')
    found.set(value.path, {
      path: value.path,
      sha256: value.sha256,
      indexBlobSha: value.indexBlobSha
    })
  }
  return ordered(found).map(([, value]) => value)
}

const LIMIT_ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/
const LIMIT_TEXT_MAX = 300
// Each limit is repeated verbatim into a consumer agent's report, so a component carries only a few.
const LIMITS_MAX = 10
const COMPONENT_NAME = /^[A-Za-z_$][\w$]*$/
// A link in any spelling the text could carry: a scheme with or without //, a bare www. host or a Figma address.
const LINK = /:\/\/|figma\.com|www\.|\b(?:javascript|mailto|data|vbscript|file|tel):/i

/**
 * Plain committed text an agent may repeat verbatim: non-empty, at most `max` characters, with no control,
 * format (bidi, zero-width) or line/paragraph separator character, no `<` or `>` and no link. The rule the
 * retirement ledger and the accepted limits share.
 * @param {unknown} text
 * @param {number} max
 * @returns {boolean}
 */
export function isPlainText(text, max) {
  return (
    typeof text === 'string' &&
    text.length > 0 &&
    text.length <= max &&
    !/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}<>]/u.test(text) &&
    !LINK.test(text)
  )
}

/** @typedef {{ id: string, text: string }} AcceptedLimit */

// A public entry is a name, or { name, limits } where limits are the owner-accepted gaps the resolver repeats to
// consumers: unique kebab-case ids and short plain sentences, never a link. Internal entries are names only.
function accepted(entry) {
  if (typeof entry === 'string') return { name: entry }
  if (!record(entry) || Object.keys(entry).some((key) => key !== 'name' && key !== 'limits'))
    throw new Error('Invalid component registry entry')
  const { name, limits } = entry
  if (typeof name !== 'string' || !COMPONENT_NAME.test(name))
    throw new Error('A component registry entry object needs a component name')
  if (!Array.isArray(limits) || !limits.length || limits.length > LIMITS_MAX)
    throw new Error(
      `A component registry entry object needs between 1 and ${LIMITS_MAX} limits: ${name}`
    )
  const ids = new Set()
  for (const limit of limits) {
    if (
      !record(limit) ||
      Object.keys(limit).length !== 2 ||
      typeof limit.id !== 'string' ||
      !LIMIT_ID.test(limit.id) ||
      limit.id.length > 48 ||
      ids.has(limit.id) ||
      !isPlainText(limit.text, LIMIT_TEXT_MAX) ||
      limit.text !== limit.text.trim()
    )
      throw new Error('Invalid or duplicate accepted limit of component ' + String(name))
    ids.add(limit.id)
  }
  return { name, limits: limits.map(({ id, text }) => ({ id, text })) }
}

/** The name of one validated registry entry, whether it is a bare name or { name, limits }. */
export function componentRegistryEntryName(entry) {
  return accepted(entry).name
}

/**
 * Names and accepted limits of a component registry (components/promotion.json), validated once for the
 * installer, the guard, the resolver and the generation record.
 * @param {unknown} registry
 * @returns {{ components: string[], internal: string[], limits: Record<string, AcceptedLimit[]> }}
 */
export function componentRegistryEntries(registry) {
  if (
    !record(registry) ||
    Object.keys(registry).some((key) => key !== 'components' && key !== 'internal') ||
    !Array.isArray(registry.components) ||
    (registry.internal !== undefined && !Array.isArray(registry.internal))
  )
    throw new Error('Invalid component visibility registry')
  if ((registry.internal ?? []).some((entry) => typeof entry !== 'string'))
    throw new Error('Internal component registry entries are names only, without limits')
  const publicEntries = registry.components.map(accepted)
  // One entry per name, compared the way visibility() compares them, so no reader keeps a later entry's limits.
  const seen = new Set()
  for (const name of [...publicEntries.map((entry) => entry.name), ...(registry.internal ?? [])]) {
    const key = String(name).normalize('NFC').toLowerCase()
    if (seen.has(key))
      throw new Error(
        'Duplicate component registry name: ' +
          (COMPONENT_NAME.test(String(name)) ? name : '(not an identifier)')
      )
    seen.add(key)
  }
  return {
    components: publicEntries.map((entry) => entry.name),
    internal: [...(registry.internal ?? [])],
    limits: Object.fromEntries(
      publicEntries.filter((entry) => entry.limits).map((entry) => [entry.name, entry.limits])
    )
  }
}

function visibility(registry) {
  const entries = componentRegistryEntries(registry)
  const all = [...entries.components, ...entries.internal]
  if (
    all.some(
      (name) => typeof name !== 'string' || !/^[A-Za-z_$][\w$]*$/.test(name) || !safe(name)
    ) ||
    new Set(all.map((name) => name.normalize('NFC').toLowerCase())).size !== all.length
  )
    throw new Error('Invalid or duplicate component visibility name')
  return { components: [...entries.components].sort(), internal: [...entries.internal].sort() }
}

// The visibility a generation record states, validated as a registry of names.
function recordedVisibility(recorded) {
  if (
    !Array.isArray(recorded.components) ||
    recorded.components.some((name) => typeof name !== 'string')
  )
    throw new Error('Invalid component generation registry names')
  return visibility({ components: recorded.components, internal: recorded.internal })
}

// The generation record names the registry's public and internal components; the accepted limits stay in the
// registry itself, which the record pins by sha256.
export function componentRegistryIdentity(bytes) {
  return {
    path: 'promotion.json',
    sha256: hash(bytes),
    ...visibility(JSON.parse(bytes.toString('utf8')))
  }
}

export function componentOutputKind(file, registry) {
  if (!safe(file)) throw new Error('Unsafe component output path')
  if (file === 'promotion.json') return 'source'
  if (file === componentGenerationFile) return 'generated'
  if (!registry.components.length && !registry.internal.length)
    throw new Error('Unexpected component output for empty registry: ' + file)
  if (RUNTIME.some((name) => file === 'runtime/' + name)) return 'runtime'
  if (file === 'runtime/figma-module-runtime.module.css.d.ts') return 'generated'
  if (/^assets\/[a-f0-9]{16}\.svg$/.test(file)) return 'generated'
  for (const name of [...registry.components, ...registry.internal]) {
    const prefix = `${name}/${name}`
    if (
      ['.tsx', '.module.css', '.module.css.d.ts'].some((extension) => file === prefix + extension)
    )
      return 'generated'
    if (registry.components.includes(name) && file === prefix + '.figma.ts') return 'generated'
  }
  throw new Error('Unexpected or internal mapping component output: ' + file)
}

function requiredOutputs(registry) {
  if (!registry.components.length && !registry.internal.length) return ['promotion.json']
  return [
    'promotion.json',
    ...RUNTIME.map((file) => 'runtime/' + file),
    'runtime/figma-module-runtime.module.css.d.ts',
    ...[...registry.components, ...registry.internal].flatMap((name) => [
      `${name}/${name}.tsx`,
      `${name}/${name}.module.css`,
      `${name}/${name}.module.css.d.ts`,
      ...(registry.components.includes(name) ? [`${name}/${name}.figma.ts`] : [])
    ])
  ]
}

function outputIdentities(values, registry) {
  if (!Array.isArray(values)) throw new Error('Invalid component output identities')
  const outputs = new Map()
  const portable = new Set()
  for (const value of values) {
    if (
      !record(value) ||
      Object.keys(value).sort().join(',') !== 'path,sha256' ||
      value.path === componentGenerationFile ||
      !digest(value.sha256)
    )
      throw new Error('Invalid or self-listed component plan output')
    componentOutputKind(value.path, registry)
    const normalized = value.path.normalize('NFC').toLowerCase()
    if (portable.has(normalized)) throw new Error('Duplicate or colliding component output')
    portable.add(normalized)
    outputs.set(value.path, { path: value.path, sha256: value.sha256 })
  }
  for (const file of requiredOutputs(registry))
    if (!outputs.has(file)) throw new Error('Missing emitted component output: ' + file)
  if (outputs.get('promotion.json').sha256 !== registry.sha256)
    throw new Error('Component registry digest differs from output')
  return ordered(outputs).map(([, value]) => value)
}

function generationRecord(source, registry, config, dependencies, modules, output) {
  return {
    schema: 2,
    source: componentSourceIdentity(source),
    registry,
    inputs: { config, dependencies },
    transform: {
      id: 'public-runtime-imports-v1',
      implementation: ROOT + 'src/install-components.ts'
    },
    modules,
    output
  }
}

const serialize = (value) => Buffer.from(JSON.stringify(value, null, 2) + '\n')

/**
 * Builds metadata from emitted bytes, never an output table's claimed origin.
 * @param {{source: object, read: (file: string) => Buffer | string, modules: readonly {path: string, sha256: string, indexBlobSha: string}[], config: {path: string, sha256: string, indexBlobSha: string}, files: ReadonlyMap<string, Buffer>, dependencies?: readonly {path: string, sha256: string, indexBlobSha: string}[]}} options
 * @returns {Buffer}
 */
export function createComponentGeneration({
  source,
  read,
  modules,
  config,
  files,
  dependencies = []
}) {
  const modulePaths = componentModulePaths(source, read)
  const verifiedModules = identities(modules, modulePaths, 'module')
  for (const module of verifiedModules)
    if (source.files[module.path].sha256 !== module.sha256)
      throw new Error('Component module differs from source lock: ' + module.path)
  const verifiedConfig = identities([config], [config?.path], 'config')[0]
  const verifiedDependencies = identities(dependencies, componentDependencyPaths, 'dependency')
  if (!(files instanceof Map) || !files.size || !Buffer.isBuffer(files.get('promotion.json')))
    throw new Error('Missing emitted component plan')
  const registry = componentRegistryIdentity(files.get('promotion.json'))
  const output = outputIdentities(
    ordered(files).map(([file, bytes]) => {
      if (file === componentGenerationFile || !Buffer.isBuffer(bytes) || !bytes.length)
        throw new Error('Invalid or self-listed component plan output')
      return { path: file, sha256: hash(bytes) }
    }),
    registry
  )
  return serialize(
    generationRecord(
      source,
      registry,
      verifiedConfig,
      verifiedDependencies,
      verifiedModules,
      output
    )
  )
}

function exactKeys(value, keys, label) {
  if (!record(value) || Object.keys(value).sort().join(',') !== [...keys].sort().join(','))
    throw new Error('Invalid component generation ' + label)
}

/**
 * Checks an offline consistency record. Actual input/output bytes still require the common inspector.
 * @param {Buffer} bytes
 * @param {object} source
 * @param {(file: string) => Buffer | string} read a locked file's bytes, which select the module read-set
 * @param {{configPath?: string, config?: {path: string, sha256: string, indexBlobSha: string}, modules?: readonly {path: string, sha256: string, indexBlobSha: string}[], dependencies?: readonly {path: string, sha256: string, indexBlobSha: string}[]}} expected
 */
export function validateComponentGeneration(bytes, source, read, expected = {}) {
  if (!Buffer.isBuffer(bytes)) throw new Error('Component generation must be bytes')
  let parsed
  try {
    parsed = JSON.parse(bytes.toString('utf8'))
  } catch (error) {
    throw new Error('Invalid component generation JSON', { cause: error })
  }
  exactKeys(
    parsed,
    ['schema', 'source', 'registry', 'inputs', 'transform', 'modules', 'output'],
    'record'
  )
  if (parsed.schema !== 2) throw new Error('Unsupported component generation schema')
  if (JSON.stringify(parsed.source) !== JSON.stringify(componentSourceIdentity(source)))
    throw new Error('Component generation source differs from lock')
  const modulePaths = componentModulePaths(source, read)
  const modules = identities(parsed.modules, modulePaths, 'module')
  for (const module of modules)
    if (source.files[module.path].sha256 !== module.sha256)
      throw new Error('Component module differs from source lock: ' + module.path)
  exactKeys(parsed.inputs, ['config', 'dependencies'], 'inputs')
  const configPath = expected.configPath ?? componentConfigPath
  const config = identities([parsed.inputs.config], [configPath], 'config')[0]
  if (
    !digest(source.files[configPath]?.sha256) ||
    source.files[configPath].sha256 !== config.sha256
  )
    throw new Error('Component configuration differs from source lock')
  const dependencies = identities(
    parsed.inputs.dependencies,
    componentDependencyPaths,
    'dependency'
  )
  const transform = {
    id: 'public-runtime-imports-v1',
    implementation: ROOT + 'src/install-components.ts'
  }
  if (JSON.stringify(parsed.transform) !== JSON.stringify(transform))
    throw new Error('Invalid component generation transform')
  exactKeys(parsed.registry, ['path', 'sha256', 'components', 'internal'], 'registry')
  if (parsed.registry.path !== 'promotion.json' || !digest(parsed.registry.sha256))
    throw new Error('Invalid component generation registry identity')
  const registry = {
    path: 'promotion.json',
    sha256: parsed.registry.sha256,
    ...recordedVisibility(parsed.registry)
  }
  const output = outputIdentities(parsed.output, registry)
  const canonical = generationRecord(source, registry, config, dependencies, modules, output)
  if (!serialize(canonical).equals(bytes))
    throw new Error('Noncanonical component generation record')
  for (const [label, values, paths] of [
    ['module', expected.modules, modulePaths],
    ['dependency', expected.dependencies, componentDependencyPaths]
  ]) {
    if (
      values !== undefined &&
      JSON.stringify(identities(values, paths, label)) !==
        JSON.stringify(label === 'module' ? modules : dependencies)
    )
      throw new Error('Component generation ' + label + ' differs from verified inputs')
  }
  if (
    expected.config !== undefined &&
    JSON.stringify(identities([expected.config], [configPath], 'config')[0]) !==
      JSON.stringify(config)
  )
    throw new Error('Component generation config differs from verified inputs')
  return canonical
}
