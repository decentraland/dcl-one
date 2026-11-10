import { createHash } from 'node:crypto'
import { lstatSync, readFileSync } from 'node:fs'
import path from 'node:path'
import {
  componentConfigPath,
  componentDependencyPaths,
  componentGenerationFile,
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

/**
 * Fixed classifications and the emitted manifest determine origins, never a lock entry's claimed kind.
 * Origins carry no module map: the table pins generation.json, whose module list is the single pin.
 */
export function componentExpectedFiles(manifest, metadataDigest) {
  const modules = Object.fromEntries(manifest.modules.map(({ path, sha256 }) => [path, sha256]))
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
                  sha256: modules[RUNTIME + file.slice('runtime/'.length)]
                }
              : generated
        return [file, { sha256: outputDigest, origin }]
      }
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
  try {
    const bytes = readFileSync(path.join(directory, componentGenerationFile))
    const registryBytes = readFileSync(path.join(directory, 'promotion.json'))
    if (sha256(bytes) !== metadata.sha256 || sha256(registryBytes) !== registry.sha256)
      return ['component metadata changed during inspection']
    const source = lock.sources?.find(
      (entry) => entry.repository === 'https://github.com/decentraland/figma-design-system'
    )
    const manifest = validateComponentGeneration(bytes, source)
    if (manifest.registry.sha256 !== registry.sha256)
      return ['component registry differs from emitted plan']
    if (
      JSON.stringify(componentRegistryIdentity(registryBytes)) !== JSON.stringify(manifest.registry)
    )
      return ['component visibility differs from emitted plan']
    const inputFiles = new Map(
      [...componentModulePaths(source), componentConfigPath].map((file) => [
        file,
        source.files[file]
      ])
    )
    for (const file of componentDependencyPaths) {
      const dependency = manifest.inputs.dependencies.find((entry) => entry.path === file)
      inputFiles.set(file, { sha256: dependency.sha256 })
    }
    const inspection = inspectPublicOutput(root, { scope: SCOPE, table, inputFiles })
    if (inspection.problems.length) return inspection.problems
    if (
      JSON.stringify(inspection.indexPaths.filter(isComponentModulePath)) !==
      JSON.stringify(componentModulePaths(source))
    )
      return ['component implementation inventory differs from source lock']
    validateComponentGeneration(bytes, source, {
      config: { path: componentConfigPath, ...inspection.inputs.get(componentConfigPath) },
      modules: componentModulePaths(source).map((file) => ({
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
    return ['invalid component generation metadata: ' + error.message]
  }
}
