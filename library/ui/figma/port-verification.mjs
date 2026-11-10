#!/usr/bin/env node
// port-verification: the checks that compare generated output with the real Figma capture cannot run in this public
// repository, because the capture lives in decentraland/figma-design-system (FDS). A pull request that changes a port
// input must carry library/ui/port-verification.json, written after those checks ran locally against the pinned
// capture (`pnpm figma fetch-source`). The port inputs are the whole lock except its provenance text (license, imported
// paths, narrative changes, deviation reasons and per-file notes), plus the bytes of every tracked file under
// library/ui/figma except tests, test fixtures and Markdown documentation. Exempt files keep their lock entry and FDS
// counterpart bound, because fetch-source compares every counterpart with the pinned FDS commit; only their own digest
// and deviation codes are left out. This check passes when the port inputs are unchanged from the base or the head has
// no lock, and otherwise only when the record names exactly the head's port inputs and every required check passed. A
// changed port input without a matching record fails; it is never skipped. The record states what a local run
// reported: it is not an attestation, a proof that the checks ran, an independent review or evidence of live Figma
// parity. Dependency-free; it needs no install, no credential and no capture.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { isComponentModulePath } from './component-generation.mjs'
import { LOCK, SCOPE, blobDigests, fdsSource } from './lock-digests.mjs'

const RECORD = 'library/ui/port-verification.json'
const SCHEMA = 2
/** Per-source lock fields that only explain provenance. Every other field, including one added later, is bound. */
const PROVENANCE_KEYS = ['license', 'imported', 'changes', 'deviationCodes']
/** Every check a record must report as passed, against the capture pinned in the lock. */
export const REQUIRED_CHECKS = [
  'fetch-source',
  'deviations',
  'diff',
  'check-mappings',
  'review-manifest',
  'navbar-bounds',
  'install'
]
const RECORD_KEYS = ['schema', 'portInputHash', 'fdsRevision', 'snapshotRevision', 'checks']
const CHECK_KEYS = ['result', 'detail']

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

/**
 * Whether a path under library/ui/figma has its bytes outside the port inputs: a test, a test fixture or Markdown
 * documentation, and never a component generation input (component-generation.mjs). port-verification.test.ts scans
 * the port inputs for path literals that point at an exempt file.
 */
export function isRecordExempt(file) {
  if (typeof file !== 'string' || !file.startsWith(`${SCOPE}/`)) return false
  if (isComponentModulePath(file)) return false
  const relative = file.slice(SCOPE.length + 1)
  return (
    /\.md$/.test(relative) ||
    /(?:^|\/)(?:test|__tests__|fixtures)\//.test(relative) ||
    /\.(?:test|spec)\.[^/]+$/.test(relative)
  )
}

/** JSON with object keys sorted at every level, so a hash does not depend on key order. */
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
  if (value && typeof value === 'object')
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(',')}}`
  return JSON.stringify(value ?? null)
}

/**
 * The hash of the port inputs, from a parsed lock and the sha256 of every tracked file under library/ui/figma keyed by
 * repository path. The lock is bound whole except PROVENANCE_KEYS and per-file notes; an exempt file's pinned entry
 * stays bound without its own digest and deviation codes, so a counterpart edit always needs a new record.
 */
export function portInputHash(lock, files) {
  const source = fdsSource(lock)
  const digests = source.snapshot?.digests
  if (!digests || typeof digests !== 'object' || Array.isArray(digests))
    throw new Error('upstream.lock.json has no capture digest table')
  if (!source.files || typeof source.files !== 'object' || Array.isArray(source.files))
    throw new Error('upstream.lock.json pins no ported files')
  const inputs = JSON.parse(JSON.stringify(lock))
  for (const entry of inputs.sources) {
    if (!entry || typeof entry !== 'object') continue
    for (const key of PROVENANCE_KEYS) delete entry[key]
    if (!entry.files || typeof entry.files !== 'object') continue
    for (const [file, pin] of Object.entries(entry.files)) {
      if (!pin || typeof pin !== 'object') continue
      delete pin.note
      if (isRecordExempt(file)) {
        delete pin.sha256
        delete pin.deviation
      }
    }
  }
  const bound = Object.keys(files)
    .filter((file) => !isRecordExempt(file))
    .sort()
    .map((file) => [file, files[file]])
  if (bound.length === 0) throw new Error(`no port input is tracked under ${SCOPE}`)
  return sha256(canonical({ schema: SCHEMA, lock: inputs, files: bound }))
}

/**
 * Validates a parsed record against the head's lock and port input hash. Returns a list of problems; empty means the
 * record reports passing checks for exactly these port inputs. Free text is limited to a short `detail` per check, so a
 * record cannot carry identifiers or names.
 */
export function recordProblems(record, { lock, inputHash }) {
  if (!record || typeof record !== 'object' || Array.isArray(record))
    return ['the record is not an object']
  const problems = []
  const keys = Object.keys(record)
  if (keys.length !== RECORD_KEYS.length || !RECORD_KEYS.every((key) => keys.includes(key)))
    problems.push(`the record must have exactly the keys ${RECORD_KEYS.join(', ')}`)
  if (record.schema !== SCHEMA) problems.push('unsupported record schema')
  if (record.portInputHash !== inputHash) problems.push('the record is for other port inputs')
  let revision
  let snapshotRevision
  try {
    const source = fdsSource(lock)
    revision = source.revision
    snapshotRevision = source.snapshot?.revision
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error))
  }
  if (typeof revision !== 'string' || record.fdsRevision !== revision)
    problems.push('the record is for another figma-design-system commit')
  if (typeof snapshotRevision !== 'string' || record.snapshotRevision !== snapshotRevision)
    problems.push('the record is for another figma-design-system capture commit')
  const checks = record.checks
  if (!checks || typeof checks !== 'object' || Array.isArray(checks))
    problems.push('the record has no checks')
  else {
    const names = Object.keys(checks)
    for (const name of REQUIRED_CHECKS)
      if (!names.includes(name)) problems.push(`check ${name} is not reported`)
    for (const name of names) {
      if (!REQUIRED_CHECKS.includes(name)) problems.push(`check ${name} is not a known check`)
      const check = checks[name]
      const checkKeys = check && typeof check === 'object' ? Object.keys(check) : []
      if (
        !check ||
        checkKeys.some((key) => !CHECK_KEYS.includes(key)) ||
        !checkKeys.includes('result')
      )
        problems.push(`check ${name} is malformed`)
      else {
        if (check.result !== 'pass') problems.push(`check ${name} did not pass`)
        if (
          check.detail !== undefined &&
          (typeof check.detail !== 'string' || !/^[\w ,.:=()/-]{0,120}$/.test(check.detail))
        )
          problems.push(`check ${name} has an unsupported detail`)
      }
    }
  }
  return problems
}

const git = (root, args, input) =>
  execFileSync('git', ['-C', root, ...args], { maxBuffer: 1 << 30, input })

function requireCommit(root, commit, role) {
  try {
    git(root, ['rev-parse', '--verify', '--quiet', `${commit}^{commit}`])
  } catch {
    throw new Error(`the ${role} ${commit} is not a commit in this checkout`)
  }
}

/**
 * The lock as of `commit`, parsed, or undefined when that commit has none: a base before the library's introduction,
 * or a head branched before it. A lock that is not valid JSON throws, so it never reads as "no lock".
 */
function lockAt(root, commit) {
  try {
    git(root, ['cat-file', '-e', `${commit}:${LOCK}`])
  } catch {
    return undefined
  }
  try {
    return JSON.parse(git(root, ['show', `${commit}:${LOCK}`]).toString('utf8'))
  } catch {
    throw new Error(`${LOCK} at ${commit} is not valid JSON`)
  }
}

/** sha256 of every file `commit` tracks under the scope, keyed by repository path. An LFS pointer or submodule throws. */
export function trackedFilesAt(root, commit) {
  const listing = git(root, ['ls-tree', '-r', '-z', '--full-tree', commit, '--', SCOPE])
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .map((entry) => {
      const [meta, file] = entry.split('\t')
      const [, type, oid] = meta.split(' ')
      if (type !== 'blob') throw new Error(`${file} is not a file`)
      return { file, oid }
    })
  return blobDigests(root, listing)
}

/** Command-line entry point. Returns the exit code. */
export function main(argv, root = process.cwd()) {
  const option = (name) => {
    const at = argv.indexOf(name)
    return at >= 0 ? argv[at + 1] : undefined
  }
  const base = option('--base')
  const head = option('--head') ?? 'HEAD'
  if (!base || argv.length !== (argv.includes('--head') ? 4 : 2)) {
    process.stderr.write('Usage: port-verification --base <commit> [--head <commit>]\n')
    return 2
  }
  try {
    const top = git(root, ['rev-parse', '--show-toplevel']).toString().trim()
    requireCommit(top, base, 'base')
    requireCommit(top, head, 'head')
    const lock = lockAt(top, head)
    // A head without the lock (a branch cut before the library existed, or one that removes it) carries no port.
    if (!lock) {
      process.stdout.write(
        'port-verification: no upstream.lock.json at this head; no port to verify\n'
      )
      return 0
    }
    const inputHash = portInputHash(lock, trackedFilesAt(top, head))
    const before = lockAt(top, base)
    // A base whose port inputs cannot be hashed (a malformed or pre-capture lock) counts as changed, never as equal.
    let baseHash
    try {
      baseHash = before && portInputHash(before, trackedFilesAt(top, base))
    } catch {
      baseHash = undefined
    }
    if (baseHash === inputHash) {
      process.stdout.write('port-verification: the port inputs are unchanged; no port to verify\n')
      return 0
    }
    try {
      git(top, ['cat-file', '-e', `${head}:${RECORD}`])
    } catch {
      process.stdout.write(
        `port-verification: the port inputs changed, and ${RECORD} is not committed\n`
      )
      return 1
    }
    let record
    try {
      record = JSON.parse(git(top, ['show', `${head}:${RECORD}`]).toString('utf8'))
    } catch {
      process.stdout.write(`port-verification: ${RECORD} is not committed valid JSON\n`)
      return 1
    }
    const problems = recordProblems(record, { lock, inputHash })
    for (const problem of problems) process.stdout.write(`port-verification: ${problem}\n`)
    if (problems.length) return 1
    process.stdout.write(
      'port-verification: the record reports passing checks for these port inputs\n'
    )
    return 0
  } catch (error) {
    process.stderr.write(
      `port-verification: ${error instanceof Error ? error.message : String(error)}\n`
    )
    return 1
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2))
}
