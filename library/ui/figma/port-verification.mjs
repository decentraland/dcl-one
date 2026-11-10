#!/usr/bin/env node
// port-verification: the checks that compare generated output with the real Figma capture cannot run in this public
// repository, because the capture lives in decentraland/figma-design-system (FDS). A pull request that changes a port
// input must carry library/ui/port-verification.json, written after those checks ran locally against the pinned
// capture (`pnpm figma fetch-source`). The port inputs are the whole lock except its provenance text (license, imported
// paths, narrative changes, deviation reasons and per-file notes), plus the bytes of every tracked file under
// library/ui/figma except tests, test fixtures and Markdown documentation. Exempt files keep their lock entry and FDS
// counterpart bound, because fetch-source compares every counterpart with the pinned FDS commit; only their own digest
// and deviation codes are left out. A head without the lock carries no port and passes. On any other head it first fails
// when a port input names an exempt file or this validator imports anything beyond SELF_IMPORTS (its two bound modules
// and four Node built-ins), whether or not a port input changed (scanProblems). It then passes when the port inputs are
// unchanged from the base, and otherwise only when the record names exactly the head's port inputs and every required check passed. A
// changed port input without a matching record fails; it is never skipped. The record states what a local run
// reported: it is not an attestation, a proof that the checks ran, an independent review or evidence of live Figma
// parity. Dependency-free; it needs no install, no credential and no capture.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { posix, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { isComponentModulePath } from './component-generation.mjs'
import { LOCK, SCOPE, digestsOf, fdsSource, parseListing, readBlobs } from './lock-digests.mjs'

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

/** The test directories under library/ui/figma; a test or fixture elsewhere is exempt only by its *.test or *.spec name. */
const TEST_ROOTS = ['test/', 'review/test/']
/** Bound although it is a test: it pins the cases the scan must flag, so weakening them needs a new record. */
const BOUND_TESTS = [`${SCOPE}/port-verification.test.ts`]
const SELF = `${SCOPE}/port-verification.mjs`
/** The only modules this file may import, so the exemption rule cannot move into an exempt file. */
const SELF_IMPORTS = [
  'node:child_process',
  'node:crypto',
  'node:path',
  'node:url',
  './component-generation.mjs',
  './lock-digests.mjs'
]
/** Literals of the validator itself that name exempt paths on purpose: the exemption rule's own declarations. */
const SELF_LITERALS = ['test/', 'review/test/', '${SCOPE}/port-verification.test.ts']
/** Port inputs whose text is scanned: code, styles, markup and the Figma tooling config. */
const SCANNED = /\.(?:[cm]?[jt]sx?|mts|cts|css|html|svg)$/
/** Data the tooling reads, scanned although JSON: its config, the retirement and orphan ledgers, and the Review catalog. */
const SCANNED_FILES = [
  `${SCOPE}/figma.config.json`,
  `${SCOPE}/ledger/orphaned-assets.json`,
  `${SCOPE}/ledger/retirements.json`,
  `${SCOPE}/review/src/review/catalog.json`
]
// A single-, double- or back-quoted literal on one line without escapes or interpolation; it also finds a literal
// nested inside a template interpolation.
const STATIC_LITERAL = /(['"`])((?:(?!\1)[^\\\n$])*)\1/g
// A single-, double- or back-quoted literal on one line; escapes and interpolations stay inside it.
const LITERAL = /(['"`])((?:\\.|(?!\1)[^\\\n])*)\1/g
// A regular expression literal after an operator or keyword. A second pass removes it so a quote inside it cannot pair
// with a later literal; the raw passes still run, so a false match here can only add candidates, never hide one.
// Comments, removed in the same pass so an apostrophe in one cannot pair with a later quote.
const COMMENTS = /\/\*[\s\S]*?\*\/|(^|[^:\\])\/\/[^\n]*/g
const REGEX_LITERAL =
  /(^|[=(,:[!&|?;>)}]|\b(?:return|typeof))(\s*)\/(?![/*])(?:\\.|\[(?:\\.|[^\]\\\n])*\]|[^/\\\n[])+\/[dgimsuyv]*/gm

/**
 * Whether a path under library/ui/figma has its bytes outside the port inputs: a file under TEST_ROOTS, a *.test or
 * *.spec file, or Markdown, and never a component generation input (component-generation.mjs) or a BOUND_TESTS file.
 * scanProblems fails a head whose port inputs name an exempt file.
 */
export function isRecordExempt(file) {
  if (typeof file !== 'string' || !file.startsWith(`${SCOPE}/`)) return false
  if (isComponentModulePath(file) || BOUND_TESTS.includes(file)) return false
  const relative = file.slice(SCOPE.length + 1)
  return (
    /\.md$/.test(relative) ||
    TEST_ROOTS.some((root) => relative.startsWith(root)) ||
    /\.(?:test|spec)\.[^/]+$/.test(relative)
  )
}

/** Whether one token (a piece with no whitespace) names an exempt file, seen from `file`. */
function tokenNamesExempt(file, token, exemptNames, markdown) {
  const bare = token.replace(/[?#].*$/, '')
  if (!bare) return false
  const pathLike = bare.startsWith('.') || bare.includes('/')
  const target = bare.startsWith('.')
    ? posix.normalize(posix.join(posix.dirname(file), bare))
    : undefined
  const base = posix.basename(bare)
  return (
    (target !== undefined && (isRecordExempt(target) || isRecordExempt(`${target}/x.ts`))) ||
    /\.(?:test|spec)\.[^/]+$/i.test(base) ||
    exemptNames.has(base) ||
    (markdown && pathLike && /\.md$/i.test(bare)) ||
    (/\*\*|\*\.[\w{]/.test(bare) && (pathLike || /\{[^}]*\bmd\b[^}]*\}/i.test(bare))) ||
    /^(?:test|fixtures|__tests__)$/.test(bare) ||
    (bare.includes('/') && /(?:^|\/)(?:test|fixtures|__tests__)(?:\/|$)/i.test(bare))
  )
}

/**
 * Whether one static piece of a literal names an exempt file: the whole piece (a path may contain spaces) or any of its
 * whitespace-separated tokens (a command line). A Markdown name counts only in a single-token piece, since prose often
 * mentions documents it never reads.
 */
function namesExempt(file, piece, exemptNames) {
  const tokens = piece.split(/\s+/).filter(Boolean)
  if (tokens.length === 0) return false
  if (tokens.length === 1) return tokenNamesExempt(file, tokens[0], exemptNames, true)
  return [piece.trim(), ...tokens].some((token) =>
    tokenNamesExempt(file, token, exemptNames, false)
  )
}

/**
 * The string literals in `text` (the source of `file`) that name an exempt file: a relative path into one, a test or
 * spec basename, the basename of a tracked exempt Markdown file, a path-like Markdown name, a path-like glob, or a test
 * directory segment. Query strings and fragments are ignored; each static piece of a template literal is checked on its
 * own. Literals are extracted four ways (with and without interpolations, each on the raw text and with regex literals
 * removed) and every way only adds candidates. This is a tripwire, not a proof: a path built by concatenation or at run
 * time is not seen, and an apostrophe in JSX text can still pair with a later quote on the same line.
 */
export function exemptReferences(file, text, exemptNames = new Set()) {
  const found = new Set()
  for (const source of [
    text,
    text.replace(COMMENTS, (comment, before = '') => before).replace(REGEX_LITERAL, '$1$2null')
  ])
    for (const [pattern, interpolated] of [
      [STATIC_LITERAL, false],
      [LITERAL, true]
    ])
      for (const match of source.matchAll(pattern)) {
        const literal = match[2]
        const pieces = interpolated && match[1] === '`' ? literal.split(/\$\{[^}]*\}/) : [literal]
        if (pieces.some((piece) => namesExempt(file, piece, exemptNames))) found.add(literal)
      }
  return [...found]
}

/**
 * Problems with the exemption at a commit, from the bytes of every tracked file under the scope: a port input's code
 * that names an exempt file, or this validator importing anything beyond SELF_IMPORTS, loading modules with require or
 * naming an exempt path outside SELF_LITERALS. Its own test spells out exempt paths on purpose and is bound, so it is not
 * scanned for literals.
 */
export function scanProblems(blobs) {
  const problems = []
  const exemptNames = new Set(
    Object.keys(blobs)
      .filter((file) => isRecordExempt(file) && /\.md$/.test(file))
      .map((file) => posix.basename(file))
  )
  for (const [file, content] of Object.entries(blobs)) {
    if (!(SCANNED.test(file) || SCANNED_FILES.includes(file)) || isRecordExempt(file)) continue
    if (BOUND_TESTS.includes(file)) continue
    const text = content.toString('utf8')
    if (file === SELF) {
      for (const match of text.matchAll(
        /(?:\bfrom\s*['"]|^import\s*['"]|\bimport\(\s*['"`])([^'"`]+)['"`]/gm
      ))
        if (!SELF_IMPORTS.includes(match[1])) problems.push(`${file} imports ${match[1]}`)
      if (/\b(?:createRequire|require)\s*\(/.test(text))
        problems.push(`${file} loads modules with require`)
    }
    for (const literal of exemptReferences(file, text, exemptNames))
      if (file !== SELF || !SELF_LITERALS.includes(literal))
        problems.push(`${file} names the exempt path ${JSON.stringify(literal)}`)
  }
  return problems
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
  if (record.portInputHash !== inputHash)
    problems.push(`the record is for other port inputs (these hash to ${inputHash})`)
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

/** The bytes of every file `commit` tracks under the scope, keyed by repository path. */
export function trackedBlobsAt(root, commit) {
  const output = git(root, ['ls-tree', '-r', '-z', '--full-tree', commit, '--', SCOPE])
  return readBlobs(root, parseListing(output, 'ls-tree'))
}

/** sha256 of every file `commit` tracks under the scope, keyed by repository path. An LFS pointer or submodule throws. */
export function trackedFilesAt(root, commit) {
  return digestsOf(trackedBlobsAt(root, commit))
}

/** Command-line entry point. Returns the exit code. */
export function main(argv, root = process.cwd()) {
  // Every argument is a known flag, each at most once, and --base and --head take a value; anything else is refused.
  const flags = {}
  let usable = true
  for (let at = 0; at < argv.length; at += 1) {
    const flag = argv[at]
    if (flag in flags || !['--base', '--head', '--print-hash'].includes(flag)) usable = false
    else if (flag === '--print-hash') flags[flag] = true
    else if (at + 1 < argv.length && !argv[at + 1].startsWith('--')) flags[flag] = argv[(at += 1)]
    else usable = false
  }
  const printHash = flags['--print-hash'] === true
  const base = flags['--base']
  const head = flags['--head'] ?? 'HEAD'
  if (!usable || (printHash ? base !== undefined : base === undefined)) {
    process.stderr.write(
      'Usage: port-verification --base <commit> [--head <commit>] | --print-hash [--head <commit>]\n'
    )
    return 2
  }
  try {
    const top = git(root, ['rev-parse', '--show-toplevel']).toString().trim()
    if (!printHash) requireCommit(top, base, 'base')
    requireCommit(top, head, 'head')
    const lock = lockAt(top, head)
    // A head without the lock (a branch cut before the library existed, or one that removes it) carries no port.
    if (!lock && printHash) {
      process.stderr.write(
        `port-verification: no upstream.lock.json at ${head}; there is no hash to print\n`
      )
      return 1
    }
    if (!lock) {
      process.stdout.write(
        'port-verification: no upstream.lock.json at this head; no port to verify\n'
      )
      return 0
    }
    const blobs = trackedBlobsAt(top, head)
    const scan = scanProblems(blobs)
    for (const problem of scan) process.stdout.write(`port-verification: ${problem}\n`)
    if (scan.length) return 1
    const inputHash = portInputHash(lock, digestsOf(blobs))
    if (printHash) {
      process.stdout.write(`${inputHash}\n`)
      return 0
    }
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
