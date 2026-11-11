import { createHash } from 'node:crypto'
import { lstatSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { inspectPublicOutput } from './public-output.mjs'

const ROOT = 'library/ui/foundations'
const FONT = { package: '@fontsource-variable/inter', version: '5.3.0', license: 'OFL-1.1' }

const CAPTURE = 'https://github.com/decentraland/figma-design-system'
const REPOSITORY = 'https://github.com/decentraland/dcl-one'

/**
 * The public foundation table the staged manifests imply, for lock-digests --write. `files` lists the staged paths
 * under library/ui/foundations (relative to it) and `readStaged` returns a non-LFS path's staged bytes. Generated
 * files take the digest source.json emitted and the pinned capture revision, the Inter fonts and license take the
 * digests font-source.json pins and the npm origin, and every other staged file is dcl-one source pinned at its
 * staged bytes, except a file `previous` pinned as generated or npm, which keeps that pin. publicFoundationProblems still checks the materialized bytes against this table.
 */
export function publicFoundationTable(lock, files, readStaged, previous) {
  const json = (file) => JSON.parse(readStaged(ROOT + '/' + file).toString('utf8'))
  const generated = json('source.json').output
  const fontSource = json('typography/font-source.json')
  if (!Array.isArray(generated) || !fontSource || typeof fontSource.files !== 'object')
    throw new Error('foundation manifests are malformed')
  const revision = lock.sources?.find((entry) => entry.repository === CAPTURE)?.snapshot?.revision
  const npm = (origin) => ({ kind: 'npm', ...FONT, path: origin })
  const table = new Map()
  for (const { path: file, sha256 } of generated)
    table.set(file, { sha256, origin: { kind: 'generated', revision } })
  for (const [name, sha256] of Object.entries(fontSource.files))
    table.set('typography/fonts/' + name, { sha256, origin: npm('files/' + name) })
  table.set('typography/' + fontSource.licenseFile, {
    sha256: fontSource.licenseSha256,
    origin: npm('LICENSE')
  })
  for (const file of files) {
    if (table.has(file)) continue
    // A file the manifests no longer list keeps a generated or npm pin, so the check reports it instead of the
    // table quietly calling it dcl-one source.
    const kind = previous?.[file]?.origin?.kind
    if (kind === 'generated' || kind === 'npm') {
      table.set(file, previous[file])
      continue
    }
    table.set(file, {
      sha256: createHash('sha256')
        .update(readStaged(ROOT + '/' + file))
        .digest('hex'),
      origin: { kind: 'source', repository: REPOSITORY }
    })
  }
  return Object.fromEntries([...table].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
}

/** Checks materialized public bytes, their index identities and declared origins without installing dependencies. */
export function publicFoundationProblems(root, lock) {
  const problems = []
  const directory = path.join(root, ROOT)
  const exists = lstatSync(directory, { throwIfNoEntry: false })
  const table = lock.publicFoundations?.files
  if (!exists && table === undefined) return problems
  if (!exists || !exists.isDirectory() || exists.isSymbolicLink())
    return ['public foundations directory is missing or unsafe']
  if (!table || typeof table !== 'object' || Array.isArray(table) || !Object.keys(table).length)
    return ['public foundations have no pinned file table']
  const lfsFiles = new Set(
    Object.keys(table).filter((file) => /^typography\/fonts\/[^/]+\.woff2$/.test(file))
  )
  const inspected = inspectPublicOutput(root, { scope: ROOT, table, lfsFiles })
  problems.push(...inspected.problems)
  const actual = inspected.actual
  const expected = Object.keys(table).sort()
  if (
    !inspected.outputs.has('typography/font-source.json') ||
    !inspected.outputs.has('source.json')
  )
    return [...problems, 'foundation manifests are missing or unsafe']
  const capture = lock.sources?.find(
    (entry) => entry.repository === 'https://github.com/decentraland/figma-design-system'
  )?.snapshot
  let fontSource
  let generated
  try {
    fontSource = JSON.parse(
      readFileSync(path.join(directory, 'typography/font-source.json'), 'utf8')
    )
    generated = JSON.parse(readFileSync(path.join(directory, 'source.json'), 'utf8')).output
    if (
      !fontSource ||
      typeof fontSource !== 'object' ||
      !Array.isArray(generated) ||
      generated.some((entry) => !entry || typeof entry.path !== 'string')
    )
      return [...problems, 'foundation manifests are malformed']
  } catch {
    return [...problems, 'foundation manifests are malformed']
  }
  if (!Object.entries(FONT).every(([key, value]) => fontSource[key] === value))
    problems.push('wrong font package origin')
  const generatedPaths = new Set(generated.map((entry) => entry.path))
  for (const output of generated)
    if (table[output.path]?.sha256 !== output.sha256)
      problems.push('generated manifest differs: ' + output.path)
  if (Object.keys(fontSource.files ?? {}).length !== 7)
    problems.push('font source must pin seven font files')
  for (const [name, digest] of Object.entries(fontSource.files ?? {}))
    if (table['typography/fonts/' + name]?.sha256 !== digest)
      problems.push('font manifest differs: ' + name)
  if (actual.filter((file) => file.endsWith('.woff2')).length !== 7)
    problems.push('public foundations must contain seven font files')
  for (const file of expected) {
    if (
      !file ||
      file.includes('\\') ||
      file.split('/').some((part) => !part || part === '.' || part === '..') ||
      path.isAbsolute(file)
    ) {
      problems.push('unsafe public file path')
      continue
    }
    const entry = table[file]
    if (!actual.includes(file)) {
      problems.push('missing public file: ' + file)
      continue
    }
    if (file.endsWith('.woff2') && fontSource.files?.[path.basename(file)] !== entry?.sha256)
      problems.push('font source digest mismatch: ' + file)
    const origin = entry?.origin
    if (generatedPaths.has(file) !== (origin?.kind === 'generated'))
      problems.push('generated origin classification differs: ' + file)
    if (origin?.kind === 'generated') {
      if (origin.revision !== capture?.revision) problems.push('wrong generated origin: ' + file)
    } else if (origin?.kind === 'npm') {
      if (
        !Object.entries(FONT).every(([key, value]) => origin[key] === value) ||
        (file.endsWith('.woff2')
          ? origin.path !== 'files/' + path.basename(file)
          : file !== 'typography/' + fontSource.licenseFile ||
            origin.path !== 'LICENSE' ||
            fontSource.licenseSha256 !== entry.sha256)
      )
        problems.push('wrong npm origin: ' + file)
    } else if (
      origin?.kind !== 'source' ||
      origin.repository !== 'https://github.com/decentraland/dcl-one'
    )
      problems.push('wrong source origin: ' + file)
    if (
      (file.endsWith('.woff2') || file === 'typography/' + fontSource.licenseFile) &&
      origin?.kind !== 'npm'
    )
      problems.push('font origin is not npm: ' + file)
  }
  return problems
}
