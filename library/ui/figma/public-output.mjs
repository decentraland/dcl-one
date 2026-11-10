import { Buffer } from 'node:buffer'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { lstatSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'

const POINTER = Buffer.from('version https://git-lfs.github.com/spec/v1')
const DIGEST = /^[0-9a-f]{64}$/
const MODES = new Set(['100644', '100755'])
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const isPointer = (bytes) => bytes.subarray(0, POINTER.length).equals(POINTER)

function portable(value) {
  return (
    typeof value === 'string' &&
    value.length > 0 &&
    !value.startsWith('/') &&
    !/[\\<>:"|?*]/.test(value) &&
    ![...value].some(
      (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
    ) &&
    value
      .split('/')
      .every(
        (part) =>
          part &&
          part !== '.' &&
          part !== '..' &&
          !/[. ]$/.test(part) &&
          !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)
      )
  )
}

function inspectPath(root, relative) {
  let current = root
  const parts = relative.split('/')
  for (let index = 0; index < parts.length; index++) {
    current = path.join(current, parts[index])
    const stat = lstatSync(current, { throwIfNoEntry: false })
    if (!stat) return 'missing'
    if (stat.isSymbolicLink()) return 'symlink'
    if (index < parts.length - 1 ? !stat.isDirectory() : !stat.isFile()) return 'special file'
  }
  return undefined
}

function filesUnder(directory, problems, prefix = '') {
  const files = []
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const relative = prefix + entry.name
    if (!portable(relative)) problems.push('unsafe public file path: ' + relative)
    else if (entry.isSymbolicLink()) problems.push('public output contains a symlink: ' + relative)
    else if (entry.isDirectory())
      files.push(...filesUnder(path.join(directory, entry.name), problems, relative + '/'))
    else if (!entry.isFile()) problems.push('public output contains a special file: ' + relative)
    else files.push(relative)
  }
  return files
}

function indexFiles(root, paths) {
  const listing = execFileSync(
    'git',
    [
      '--no-replace-objects',
      '--literal-pathspecs',
      '-C',
      root,
      'ls-files',
      '-s',
      '-z',
      '--',
      ...paths
    ],
    { maxBuffer: 1 << 28 }
  )
  const entries = new Map()
  for (const line of listing.toString('utf8').split('\0').filter(Boolean)) {
    const tab = line.indexOf('\t')
    const [mode, oid, stage] = line.slice(0, tab).split(' ')
    const file = line.slice(tab + 1)
    const previous = entries.get(file)
    entries.set(file, { mode, oid, unmerged: stage !== '0' || Boolean(previous) })
  }
  return entries
}

function indexBlobs(root, entries) {
  const ids = [
    ...new Set(
      [...entries.values()]
        .filter((entry) => !entry.unmerged && MODES.has(entry.mode))
        .map((entry) => entry.oid)
    )
  ]
  if (!ids.length) return new Map()
  const output = execFileSync('git', ['--no-replace-objects', '-C', root, 'cat-file', '--batch'], {
    input: ids.join('\n') + '\n',
    maxBuffer: 1 << 30
  })
  const blobs = new Map()
  let offset = 0
  for (const oid of ids) {
    const end = output.indexOf(0x0a, offset)
    const [found, kind, size] = output.subarray(offset, end).toString('utf8').split(' ')
    if (end < 0 || found !== oid || kind !== 'blob' || !/^\d+$/.test(size))
      throw new Error('invalid index blob')
    const start = end + 1
    const finish = start + Number(size)
    if (finish >= output.length || output[finish] !== 0x0a) throw new Error('truncated index blob')
    blobs.set(oid, output.subarray(start, finish))
    offset = finish + 1
  }
  return blobs
}

/**
 * Inspect only independently supplied paths and pins. Origin claims never select filesystem inputs.
 * @param {string} root
 * @param {{scope: string, table: Record<string, {sha256: string, origin?: unknown}>, lfsFiles?: ReadonlySet<string>, inputFiles?: ReadonlyMap<string, {sha256: string}>}} options
 * @returns {{problems: string[], outputs: Map<string, {sha256: string, indexBlobSha: string, origin: unknown}>, inputs: Map<string, {sha256: string, indexBlobSha: string}>, actual: string[], indexPaths: string[]}}
 */
export function inspectPublicOutput(
  root,
  { scope, table, lfsFiles = new Set(), inputFiles = new Map() }
) {
  const problems = []
  const outputs = new Map()
  const inputs = new Map()
  const result = { problems, outputs, inputs, actual: [], indexPaths: [] }
  if (!['library/ui/foundations', 'library/ui/components'].includes(scope)) {
    problems.push('unsupported public output scope')
    return result
  }
  if (!table || typeof table !== 'object' || Array.isArray(table) || !Object.keys(table).length) {
    problems.push('public output has no pinned file table')
    return result
  }
  const expected = Object.keys(table).sort()
  const safeExpected = expected.filter((file) => {
    if (portable(file)) return true
    problems.push('unsafe public file path')
    return false
  })
  const allowedLfs = scope === 'library/ui/foundations' ? lfsFiles : new Set()
  if (scope === 'library/ui/components' && lfsFiles.size)
    problems.push('components do not permit LFS exceptions')
  for (const file of allowedLfs)
    if (
      !portable(file) ||
      !/^typography\/fonts\/[^/]+\.woff2$/.test(file) ||
      !Object.hasOwn(table, file)
    )
      problems.push('unsafe foundation LFS exception: ' + file)
  const safeInputs = []
  for (const [file, entry] of inputFiles) {
    if (!portable(file) || !DIGEST.test(entry?.sha256 ?? ''))
      problems.push('unsafe or unpinned public input: ' + file)
    else safeInputs.push(file)
  }
  try {
    // Check directory ancestors without traversing a symlink before listing any children.
    const rootStat = lstatSync(root, { throwIfNoEntry: false })
    if (!rootStat || rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
      problems.push('public output directory is missing or unsafe')
      return result
    }
    let directory = root
    for (const part of scope.split('/')) {
      directory = path.join(directory, part)
      const stat = lstatSync(directory, { throwIfNoEntry: false })
      if (!stat || stat.isSymbolicLink() || !stat.isDirectory()) {
        problems.push('public output directory is missing or unsafe')
        return result
      }
    }
    result.actual = filesUnder(directory, problems).sort()
    const actual = new Set(result.actual)
    const collisions = new Set()
    for (const file of new Set([...expected, ...result.actual])) {
      const normalized = file.normalize('NFC').toLowerCase()
      if (collisions.has(normalized)) problems.push('non-portable public file collision: ' + file)
      collisions.add(normalized)
    }
    // Inventory executable source independently of a manifest's claimed input list.
    const sourceScope =
      scope === 'library/ui/components' && inputFiles.size ? ['library/ui/figma/src'] : []
    const index = indexFiles(root, [scope, ...safeInputs, ...sourceScope])
    result.indexPaths = [...index.keys()].sort()
    const blobs = indexBlobs(root, index)
    for (const file of result.actual)
      if (!Object.hasOwn(table, file)) problems.push('extra public file: ' + file)
    for (const file of index.keys()) {
      if (file.startsWith(scope + '/') && !Object.hasOwn(table, file.slice(scope.length + 1)))
        problems.push('extra indexed public file: ' + file.slice(scope.length + 1))
    }
    function identity(file, pin, label, lfs) {
      const indexed = index.get(file)
      if (!indexed) {
        problems.push('untracked ' + label)
        return undefined
      }
      if (indexed.unmerged) {
        problems.push('unmerged ' + label)
        return undefined
      }
      if (!MODES.has(indexed.mode)) {
        problems.push('unsafe index mode for ' + label)
        return undefined
      }
      const unsafe = inspectPath(root, file)
      if (unsafe) {
        problems.push(unsafe + ' ' + label)
        return undefined
      }
      const bytes = readFileSync(path.join(root, file))
      if (isPointer(bytes)) {
        problems.push('unmaterialized ' + label)
        return undefined
      }
      const digest = sha256(bytes)
      const blob = blobs.get(indexed.oid)
      let valid = true
      if (!DIGEST.test(pin ?? '') || digest !== pin) {
        problems.push('changed ' + label)
        valid = false
      }
      if (lfs) {
        const match =
          /^version https:\/\/git-lfs.github.com\/spec\/v1\noid sha256:([0-9a-f]{64})\nsize (0|[1-9][0-9]*)\n$/.exec(
            blob.toString('utf8')
          )
        if (
          !match ||
          match[1] !== pin ||
          match[1] !== digest ||
          match[2] !== String(bytes.length)
        ) {
          problems.push('font LFS identity mismatch: ' + file.slice(scope.length + 1))
          valid = false
        }
      } else if (isPointer(blob) || !blob.equals(bytes)) {
        problems.push(
          label.startsWith('public file: ')
            ? 'public index bytes differ: ' + label.slice('public file: '.length)
            : 'input index bytes differ: ' + file
        )
        valid = false
      }
      return valid ? { sha256: digest, indexBlobSha: indexed.oid } : undefined
    }
    for (const file of safeExpected) {
      if (!actual.has(file)) {
        problems.push('missing public file: ' + file)
        continue
      }
      const found = identity(
        scope + '/' + file,
        table[file]?.sha256,
        'public file: ' + file,
        allowedLfs.has(file)
      )
      if (found) outputs.set(file, { ...found, origin: table[file]?.origin })
    }
    for (const file of safeInputs) {
      const found = identity(file, inputFiles.get(file).sha256, 'public input: ' + file, false)
      if (found) inputs.set(file, found)
    }
  } catch (error) {
    problems.push(
      'public output inspection failed: ' + (error instanceof Error ? error.name : 'unknown error')
    )
  }
  return result
}
