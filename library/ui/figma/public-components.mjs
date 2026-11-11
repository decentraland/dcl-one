import { createHash } from 'node:crypto'
import { lstatSync, readFileSync } from 'node:fs'
import path from 'node:path'
import {
  componentConfigPath,
  componentDependencyPaths,
  componentGenerationFile,
  componentModuleCandidates,
  componentModulePaths,
  componentOutputKind,
  componentRegistryIdentity,
  isComponentModulePath,
  validateComponentGeneration
} from './component-generation.mjs'
import { inspectPublicOutput } from './public-output.mjs'
import { publicComponentProblems } from './public-components-policy.mjs'

const SCOPE = 'library/ui/components'
const RUNTIME = 'library/ui/figma/review/src/components/'
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
class InputProblem extends Error {
  constructor(problem, file) {
    super(problem + ': ' + file)
    this.file = file
  }
}

/**
 * Fixed classifications and the emitted manifest determine origins, never a lock entry's claimed kind.
 * Origins carry no module map: the table pins generation.json by digest, and that record's module list is
 * validated against each module's digest in the source lock.
 */
export function componentExpectedFiles(manifest, metadataDigest) {
  // Only to look up the runtime files' digests; origins never carry the module list.
  const moduleDigests = Object.fromEntries(
    manifest.modules.map(({ path, sha256 }) => [path, sha256])
  )
  const generated = {
    kind: 'generated',
    source: JSON.stringify(manifest.source),
    transform: manifest.transform.id
  }
  return new Map(
    [...manifest.output, { path: componentGenerationFile, sha256: metadataDigest }].map(
      ({ path: file, sha256: outputDigest }) => {
        const kind = componentOutputKind(file, manifest.registry)
        const origin =
          kind === 'source'
            ? { kind, repository: 'https://github.com/decentraland/dcl-one' }
            : kind === 'runtime'
              ? {
                  ...generated,
                  kind,
                  path: RUNTIME + file.slice('runtime/'.length),
                  sha256: moduleDigests[RUNTIME + file.slice('runtime/'.length)]
                }
              : generated
        return [file, { sha256: outputDigest, origin }]
      }
    )
  )
}

/**
 * The public component table the staged generation record implies, for lock-digests --write: its emitted outputs and
 * the record itself, with origins from componentExpectedFiles. `readStaged` returns a path's staged bytes. The record
 * is validated against the (already resealed) source lock first, so a stale record throws instead of being pinned.
 */
export function publicComponentTable(lock, readStaged) {
  const source = lock.sources?.find(
    (entry) => entry.repository === 'https://github.com/decentraland/figma-design-system'
  )
  if (!source) throw new Error('missing component source pin')
  const bytes = readStaged(SCOPE + '/' + componentGenerationFile)
  const read = (file) => {
    const content = readStaged(file)
    if (sha256(content) !== source.files?.[file]?.sha256)
      throw new Error('staged component input differs from the source lock: ' + file)
    return content
  }
  const manifest = validateComponentGeneration(bytes, source, read)
  return Object.fromEntries(
    [...componentExpectedFiles(manifest, sha256(bytes))].sort(([a], [b]) =>
      a < b ? -1 : a > b ? 1 : 0
    )
  )
}

/** Offline byte/module consistency, not an attestation of live Figma parity. */
export function publicComponentsProblems(root, lock) {
  const directory = path.join(root, SCOPE)
  const exists = lstatSync(directory, { throwIfNoEntry: false })
  const table = lock.publicComponents?.files
  if (!exists && table === undefined) return []
  // The inspector validates ancestors, index modes and every output before any metadata is read.
  const initial = inspectPublicOutput(root, { scope: SCOPE, table })
  if (initial.problems.length) return initial.problems
  const metadata = initial.outputs.get(componentGenerationFile)
  const registry = initial.outputs.get('promotion.json')
  if (!metadata || !registry) return ['missing component generation metadata or registry']
  const source = lock.sources?.find(
    (entry) => entry.repository === 'https://github.com/decentraland/figma-design-system'
  )
  try {
    const bytes = readFileSync(path.join(directory, componentGenerationFile))
    const registryBytes = readFileSync(path.join(directory, 'promotion.json'))
    if (sha256(bytes) !== metadata.sha256 || sha256(registryBytes) !== registry.sha256)
      return ['component metadata changed during inspection']
    // Module bytes select the read-set, so a missing, unsafe or changed module is reported in the inspector's own
    // words before any of them is trusted.
    const read = (file) => {
      const full = path.join(root, file)
      const stat = lstatSync(full, { throwIfNoEntry: false })
      if (!stat) throw new InputProblem('missing public input', file)
      if (!stat.isFile()) throw new InputProblem('unsafe public input', file)
      const content = readFileSync(full)
      if (sha256(content) !== source.files[file]?.sha256)
        throw new InputProblem('changed public input', file)
      return content
    }
    const manifest = validateComponentGeneration(bytes, source, read)
    const modulePaths = componentModulePaths(source, read)
    if (manifest.registry.sha256 !== registry.sha256)
      return ['component registry differs from emitted plan']
    if (
      JSON.stringify(componentRegistryIdentity(registryBytes)) !== JSON.stringify(manifest.registry)
    )
      return ['component visibility differs from emitted plan']
    const inputFiles = new Map(
      [...modulePaths, componentConfigPath].map((file) => [file, source.files[file]])
    )
    for (const file of componentDependencyPaths) {
      const dependency = manifest.inputs.dependencies.find((entry) => entry.path === file)
      inputFiles.set(file, { sha256: dependency.sha256 })
    }
    const inspection = inspectPublicOutput(root, { scope: SCOPE, table, inputFiles })
    if (inspection.problems.length) return inspection.problems
    if (
      JSON.stringify(inspection.indexPaths.filter(isComponentModulePath)) !==
      JSON.stringify(componentModuleCandidates(source))
    )
      return ['component implementation inventory differs from source lock']
    validateComponentGeneration(bytes, source, read, {
      config: { path: componentConfigPath, ...inspection.inputs.get(componentConfigPath) },
      modules: modulePaths.map((file) => ({
        path: file,
        ...inspection.inputs.get(file)
      })),
      dependencies: componentDependencyPaths.map((file) => ({
        path: file,
        ...inspection.inputs.get(file)
      }))
    })
    const expected = componentExpectedFiles(manifest, metadata.sha256)
    return publicComponentProblems({
      inspection,
      expected,
      source: JSON.stringify(manifest.source),
      sourceFiles: source.files,
      transformModules: new Map(manifest.modules.map(({ path, sha256 }) => [path, sha256]))
    })
  } catch (error) {
    if (error instanceof InputProblem) {
      // The inspector's full report on that one input (materialized and index identity), never a shorter one.
      const inputFiles = new Map([[error.file, { sha256: source.files[error.file]?.sha256 }]])
      const detail = inspectPublicOutput(root, { scope: SCOPE, table, inputFiles }).problems.filter(
        (problem) => problem.endsWith(': ' + error.file)
      )
      return detail.length ? detail : [error.message]
    }
    return ['invalid component generation metadata: ' + error.message]
  }
}
