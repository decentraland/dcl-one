const INSTALLER = 'library/ui/figma/src/install-components.ts'
const TRANSFORM = 'public-runtime-imports-v1'
const RUNTIME_ROOT = 'library/ui/figma/review/src/components/'
const RUNTIME = new Set(
  [
    'figma-module-runtime.tsx',
    'figma-module-runtime.module.css',
    'figma-view-state.ts',
    'figma-interactions.tsx'
  ].map((file) => RUNTIME_ROOT + file)
)
const REPOSITORY = 'https://github.com/decentraland/dcl-one'
const digest = (value) => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value)
const record = (value) =>
  value !== null &&
  typeof value === 'object' &&
  !Array.isArray(value) &&
  [Object.prototype, null].includes(Object.getPrototypeOf(value))
const safe = (value) =>
  typeof value === 'string' &&
  !value.includes('\\') &&
  !value.includes(':') &&
  ![...value].some(
    (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
  ) &&
  value
    .split('/')
    .every((part) => part && !['.', '..', '__proto__', 'prototype', 'constructor'].includes(part))
const exactKeys = (value, keys) =>
  record(value) &&
  Object.keys(value).length === keys.length &&
  keys.every((key) => Object.hasOwn(value, key))

/**
 * Pure policy over verified identities and an independently emitted plan. The caller must
 * acquire and verify inputs before calling; the opaque source identity is not a receipt.
 * Filesystem, index, LFS and public table verification belong to the common inspector.
 *
 * @param {{inspection?: {problems: string[], outputs: ReadonlyMap<string, {sha256: string, indexBlobSha: string, origin: unknown}>, inputs: ReadonlyMap<string, {sha256: string, indexBlobSha: string}>}, expected?: ReadonlyMap<string, {sha256: string, origin: Record<string, unknown>}>, source?: string, sourceFiles?: Record<string, {sha256: string}>, transformModules?: ReadonlyMap<string, string>}} context
 * @returns {string[]}
 */
export function publicComponentProblems(context = {}) {
  const { inspection, expected, source, sourceFiles, transformModules } = context
  if (
    !inspection ||
    !Array.isArray(inspection.problems) ||
    !(inspection.outputs instanceof Map) ||
    !(inspection.inputs instanceof Map)
  )
    return ['missing verified component inspection']
  const problems = [...inspection.problems]
  if (!(expected instanceof Map) || !expected.size)
    return [...problems, 'missing independent component plan']
  if (typeof source !== 'string' || !source.trim())
    problems.push('missing component source identity')
  if (!record(sourceFiles)) problems.push('missing locked component source files')
  // The module map is pinned once, by the table's digest of generation.json; origins never repeat it.
  const modulesValid =
    transformModules instanceof Map &&
    transformModules.size > 0 &&
    transformModules.has(INSTALLER) &&
    [...transformModules].every(
      ([file, hash]) =>
        safe(file) &&
        digest(hash) &&
        Object.hasOwn(sourceFiles ?? {}, file) &&
        sourceFiles[file]?.sha256 === hash &&
        inspection.inputs.get(file)?.sha256 === hash &&
        Boolean(inspection.inputs.get(file)?.indexBlobSha)
    )
  if (!modulesValid) problems.push('missing or mismatched component transform identities')
  const originValid = (file, origin) => {
    if (file === 'promotion.json')
      return (
        exactKeys(origin, ['kind', 'repository']) &&
        origin.kind === 'source' &&
        origin.repository === REPOSITORY
      )
    if (!record(origin) || !['generated', 'runtime'].includes(origin.kind)) return false
    if (
      !exactKeys(
        origin,
        origin.kind === 'runtime'
          ? ['kind', 'source', 'transform', 'path', 'sha256']
          : ['kind', 'source', 'transform']
      )
    )
      return false
    if (
      typeof source !== 'string' ||
      !source.trim() ||
      origin.source !== source ||
      origin.transform !== TRANSFORM
    )
      return false
    if (origin.kind === 'generated') return true
    return (
      safe(origin.path) &&
      RUNTIME.has(origin.path) &&
      file === 'runtime/' + origin.path.slice(RUNTIME_ROOT.length) &&
      Object.hasOwn(sourceFiles ?? {}, origin.path) &&
      digest(origin.sha256) &&
      sourceFiles[origin.path]?.sha256 === origin.sha256 &&
      inspection.inputs.get(origin.path)?.sha256 === origin.sha256 &&
      Boolean(inspection.inputs.get(origin.path)?.indexBlobSha)
    )
  }
  for (const file of inspection.outputs.keys())
    if (!expected.has(file)) problems.push('unexpected public component: ' + file)
  for (const [file, emitted] of expected) {
    if (!safe(file)) {
      problems.push('unsafe component plan path: ' + file)
      continue
    }
    const actual = inspection.outputs.get(file)
    if (!actual) {
      problems.push('missing public component: ' + file)
      continue
    }
    if (!digest(emitted?.sha256) || actual.sha256 !== emitted.sha256)
      problems.push('emitted component digest differs: ' + file)
    if (!actual.indexBlobSha) problems.push('missing verified component index identity: ' + file)
    // Classification comes from the independently emitted plan, never the output table.
    if (!originValid(file, emitted?.origin)) {
      problems.push('invalid independent component origin: ' + file)
      continue
    }
    const claim = actual.origin
    if (
      !originValid(file, claim) ||
      !Object.keys(emitted.origin).every((key) => emitted.origin[key] === claim[key])
    )
      problems.push('component origin differs: ' + file)
  }
  return problems
}
