#!/usr/bin/env node
// port-verification: a pull request that changes upstream.lock.json is a port from decentraland/figma-design-system
// (FDS), and the checks that compare generated output with the real Figma capture cannot run in this public
// repository: the capture lives in FDS. Such a pull request must carry library/ui/port-verification.json, written
// after those checks ran locally against the pinned capture (`pnpm figma fetch-source`). This check passes when the
// lock is unchanged from the base or absent at the head, and otherwise only when the record names this exact lock,
// FDS commit and library/ui/figma tree and every required check passed. A lock change without a matching record fails; it is never
// skipped. Dependency-free; it needs no install, no credential and no capture.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { fdsSource } from './lock-digests.mjs'

const LOCK = 'library/ui/upstream.lock.json'
const RECORD = 'library/ui/port-verification.json'
const SCOPE = 'library/ui/figma'
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
const RECORD_KEYS = ['schema', 'lockHash', 'fdsRevision', 'snapshotRevision', 'figmaTree', 'checks']
const CHECK_KEYS = ['result', 'detail']

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

/**
 * Validates a parsed record against the current lock bytes and library/ui/figma tree. Returns a list of problems;
 * empty means the record vouches for exactly this port. Free text is limited to a short `detail` per check, so a
 * record cannot carry identifiers or names.
 */
export function recordProblems(record, { lockBytes, figmaTree }) {
  if (!record || typeof record !== 'object' || Array.isArray(record))
    return ['the record is not an object']
  const problems = []
  const keys = Object.keys(record)
  if (keys.length !== RECORD_KEYS.length || !RECORD_KEYS.every((key) => keys.includes(key)))
    problems.push(`the record must have exactly the keys ${RECORD_KEYS.join(', ')}`)
  if (record.schema !== 1) problems.push('unsupported record schema')
  if (record.lockHash !== sha256(lockBytes))
    problems.push('the record is for another upstream.lock.json')
  let revision
  let snapshotRevision
  try {
    const source = fdsSource(JSON.parse(lockBytes.toString('utf8')))
    revision = source.revision
    snapshotRevision = source.snapshot?.revision
  } catch (error) {
    problems.push(error instanceof Error ? error.message : String(error))
  }
  if (record.fdsRevision !== revision)
    problems.push('the record is for another figma-design-system commit')
  if (typeof snapshotRevision !== 'string' || record.snapshotRevision !== snapshotRevision)
    problems.push('the record is for another figma-design-system capture commit')
  if (record.figmaTree !== figmaTree) problems.push(`the record is for another ${SCOPE} tree`)
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

const git = (root, args) => execFileSync('git', ['-C', root, ...args], { maxBuffer: 1 << 28 })

/**
 * The lock as of `commit`, or undefined when that commit has none: a base before the library's introduction, or a
 * head branched before it. A name that is not a commit throws, so a typo never reads as "no lock".
 */
function lockAt(root, commit, role) {
  try {
    git(root, ['rev-parse', '--verify', '--quiet', `${commit}^{commit}`])
  } catch {
    throw new Error(`the ${role} ${commit} is not a commit in this checkout`)
  }
  try {
    git(root, ['cat-file', '-e', `${commit}:${LOCK}`])
  } catch {
    return undefined
  }
  return git(root, ['show', `${commit}:${LOCK}`])
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
    const before = lockAt(top, base, 'base')
    const lockBytes = lockAt(top, head, 'head')
    // A head without the lock (a branch cut before the library existed, or one that removes it) carries no port.
    if (!lockBytes) {
      process.stdout.write(
        'port-verification: no upstream.lock.json at this head; no port to verify\n'
      )
      return 0
    }
    if (before && before.equals(lockBytes)) {
      process.stdout.write(
        'port-verification: upstream.lock.json is unchanged; no port to verify\n'
      )
      return 0
    }
    try {
      git(top, ['cat-file', '-e', `${head}:${RECORD}`])
    } catch {
      process.stdout.write(
        `port-verification: upstream.lock.json changed, and ${RECORD} is not committed\n`
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
    const figmaTree = git(top, ['rev-parse', `${head}:${SCOPE}`])
      .toString()
      .trim()
    const problems = recordProblems(record, { lockBytes, figmaTree })
    for (const problem of problems) process.stdout.write(`port-verification: ${problem}\n`)
    if (problems.length) return 1
    process.stdout.write(
      'port-verification: the record vouches for this lock, FDS commit and tree\n'
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
