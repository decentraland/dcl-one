import { Buffer } from 'node:buffer'
import { createHash } from 'node:crypto'
import { componentSourceIdentity } from './source-identity.mjs'

const ROOT = 'library/ui/figma/'
const RUNTIME = [
  'figma-module-runtime.tsx',
  'figma-module-runtime.module.css',
  'figma-view-state.ts',
  'figma-interactions.tsx'
]
const MODULES = [
  ROOT + 'component-generation.mjs',
  ROOT + 'source-identity.mjs',
  ROOT + 'ledger/retirements.json',
  ROOT + 'tsconfig.json',
  ROOT + 'src/install.ts',
  ROOT + 'src/install-components.ts',
  ROOT + 'src/generators/components.ts',
  ROOT + 'src/generators/tokens.ts',
  ...RUNTIME.map((file) => ROOT + 'review/src/components/' + file)
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

/** Implementation, resolver configuration and the effective retirement ledger are generation inputs. */
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

/** A conservative complete read-set, chosen independently of manifest claims. */
export function componentModulePaths(source) {
  const files = source?.files
  if (!record(files)) throw new Error('No locked component module table')
  const paths = Object.keys(files).filter(isComponentModulePath)
  for (const file of MODULES)
    if (!paths.includes(file)) throw new Error('Missing locked component implementation: ' + file)
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

// A public entry is a name, or { name, limits } where limits are the owner-accepted gaps the resolver repeats to
// consumers: unique kebab-case ids and short plain sentences, never a link. Internal entries are names only.
function accepted(entry) {
  if (typeof entry === 'string') return { name: entry }
  if (!record(entry) || Object.keys(entry).some((key) => key !== 'name' && key !== 'limits'))
    throw new Error('Invalid component registry entry')
  const { name, limits } = entry
  if (!Array.isArray(limits) || !limits.length)
    throw new Error('A component registry entry object needs a non-empty limits list')
  const ids = new Set()
  for (const limit of limits) {
    if (
      !record(limit) ||
      Object.keys(limit).length !== 2 ||
      typeof limit.id !== 'string' ||
      !LIMIT_ID.test(limit.id) ||
      limit.id.length > 48 ||
      ids.has(limit.id) ||
      typeof limit.text !== 'string' ||
      !limit.text.trim() ||
      limit.text !== limit.text.trim() ||
      limit.text.length > LIMIT_TEXT_MAX ||
      limit.text.includes('://') ||
      [...limit.text].some(
        (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
      )
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
 * installer, the guard and the generation record.
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
  return {
    components: [...entries.components].sort(),
    internal: [...entries.internal].sort(),
    limits: Object.fromEntries(
      Object.entries(entries.limits).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    )
  }
}

// The visibility a generation record states, rebuilt into registry form so the same validation applies.
function recordedVisibility(recorded) {
  if (!record(recorded.limits)) throw new Error('Invalid component generation registry limits')
  const limited = new Map(Object.entries(recorded.limits))
  if (
    !Array.isArray(recorded.components) ||
    [...limited.keys()].some((name) => !recorded.components.includes(name))
  )
    throw new Error('Component generation limits name a component that is not public')
  return visibility({
    components: recorded.components.map((name) =>
      limited.has(name) ? { name, limits: limited.get(name) } : name
    ),
    internal: recorded.internal
  })
}

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
    schema: 1,
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
 * @param {{source: object, modules: readonly {path: string, sha256: string, indexBlobSha: string}[], config: {path: string, sha256: string, indexBlobSha: string}, files: ReadonlyMap<string, Buffer>, dependencies?: readonly {path: string, sha256: string, indexBlobSha: string}[]}} options
 * @returns {Buffer}
 */
export function createComponentGeneration({ source, modules, config, files, dependencies = [] }) {
  const modulePaths = componentModulePaths(source)
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
 * @param {{configPath?: string, config?: {path: string, sha256: string, indexBlobSha: string}, modules?: readonly {path: string, sha256: string, indexBlobSha: string}[], dependencies?: readonly {path: string, sha256: string, indexBlobSha: string}[]}} expected
 */
export function validateComponentGeneration(bytes, source, expected = {}) {
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
  if (parsed.schema !== 1) throw new Error('Unsupported component generation schema')
  if (JSON.stringify(parsed.source) !== JSON.stringify(componentSourceIdentity(source)))
    throw new Error('Component generation source differs from lock')
  const modulePaths = componentModulePaths(source)
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
  exactKeys(parsed.registry, ['path', 'sha256', 'components', 'internal', 'limits'], 'registry')
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
