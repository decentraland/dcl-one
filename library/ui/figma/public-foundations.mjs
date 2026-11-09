import { Buffer } from 'node:buffer'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { lstatSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'

const ROOT = 'library/ui/foundations'
const POINTER = Buffer.from('version https://git-lfs.github.com/spec/v1\n')
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const FONT = { package: '@fontsource-variable/inter', version: '5.3.0', license: 'OFL-1.1' }

function filesUnder(directory, prefix = '') {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const relative = prefix + entry.name
    if (entry.isSymbolicLink()) throw new Error('Public foundations contain a symlink')
    if (entry.isDirectory()) return filesUnder(path.join(directory, entry.name), relative + '/')
    if (!entry.isFile()) throw new Error('Public foundations contain a special file')
    return [relative]
  })
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
  const actual = filesUnder(directory).sort()
  const expected = Object.keys(table).sort()
  const indexed = new Set(
    execFileSync('git', ['-C', root, 'ls-files', '--', ROOT], { encoding: 'utf8' })
      .trim()
      .split('\n')
      .filter(Boolean)
  )
  for (const file of actual)
    if (!Object.hasOwn(table, file)) problems.push('extra public file: ' + file)
  const capture = lock.sources?.find(
    (entry) => entry.repository === 'https://github.com/decentraland/figma-design-system'
  )?.snapshot
  const fontSource = JSON.parse(
    readFileSync(path.join(directory, 'typography/font-source.json'), 'utf8')
  )
  if (!Object.entries(FONT).every(([key, value]) => fontSource[key] === value))
    problems.push('wrong font package origin')
  const generated = JSON.parse(readFileSync(path.join(directory, 'source.json'), 'utf8')).output
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
    if (!indexed.has(ROOT + '/' + file)) {
      problems.push('untracked public file: ' + file)
      continue
    }
    const bytes = readFileSync(path.join(directory, file))
    if (bytes.subarray(0, POINTER.length).equals(POINTER)) {
      problems.push('unmaterialized public file: ' + file)
      continue
    }
    if (!/^[0-9a-f]{64}$/.test(entry?.sha256 ?? '') || sha256(bytes) !== entry.sha256)
      problems.push('changed public file: ' + file)
    const blob = execFileSync('git', ['-C', root, 'show', ':' + ROOT + '/' + file])
    const pointer = blob.subarray(0, POINTER.length).equals(POINTER)
    if (file.endsWith('.woff2')) {
      const match =
        /^version https:\/\/git-lfs.github.com\/spec\/v1\noid sha256:([0-9a-f]{64})\nsize ([0-9]+)\n$/.exec(
          blob.toString('utf8')
        )
      if (!match || match[1] !== entry.sha256 || Number(match[2]) !== bytes.length)
        problems.push('font LFS identity mismatch: ' + file)
      if (fontSource.files?.[path.basename(file)] !== entry.sha256)
        problems.push('font source digest mismatch: ' + file)
    } else if (pointer || !blob.equals(bytes)) problems.push('public index bytes differ: ' + file)
    const origin = entry.origin
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
