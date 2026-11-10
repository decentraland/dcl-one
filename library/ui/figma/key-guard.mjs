#!/usr/bin/env node
// figma-key-guard: fails when the private Figma file key, or any Figma file URL, is present in
// the working tree, in a blob of the outgoing commit range, in a tree read from the object store, or in a
// generated output directory. --tree and --allowlist-from read another revision's files as data only, so a
// trusted copy of this script can scan a pull request without checking it out or running any of its code.
// Diagnostics name the path (or commit:path) and the check ID only, never the match or the key.
// Dependency-free so it runs before `pnpm install` and in fork CI.
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { lstatSync, readFileSync, readdirSync, realpathSync, statSync } from 'node:fs'
import { join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

// A Figma URL in any case, under any path segment (design, file, proto, board, slides, make, site, deck…),
// with its separators written plainly, percent-encoded (%2F, as in embed links) or JSON-escaped (\/). The
// lookbehind keeps hosts that merely end in "figma.com" out; subdomains such as www. still match. REST API
// paths (/v1/…) are not file links: a real key in one still fails the key check.
const SEPARATOR = String.raw`(?:\/|%2f|\\+\/|\\+u002f)`
export const URL_PATTERN = new RegExp(
  String.raw`(?<![a-z0-9-])figma\.com${SEPARATOR}(?!v\d+${SEPARATOR})[a-z0-9_-]+${SEPARATOR}([a-z0-9]+)`,
  'gi'
)
const LFS_POINTER_PREFIX = 'version https://git-lfs'
const KEY_PLACEHOLDER = '<DCL_FIGMA_FILE>'

/**
 * Replaces the private file key, in any letter case, in a string that came from untrusted data: a path in a
 * pull request's tree, range or artifact, a revision name, or the text of an error that quotes one. Every
 * string this script stores or prints goes through here, so a file named after the key cannot leak it.
 */
export function redact(text, key) {
  const value = typeof text === 'string' ? text : String(text)
  if (!key) return value
  return value.replace(
    new RegExp(key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi'),
    KEY_PLACEHOLDER
  )
}

// output:begin
// The only place this script writes to stdout or stderr: nothing reaches either without being redacted.
function output(stream, text, key) {
  ;(stream === 'stderr' ? process.stderr : process.stdout).write(`${redact(text, key)}\n`)
}
// output:end
const ALLOWLIST_PATH = 'library/ui/figma/key-guard-allow.json'

function git(root, args, input) {
  return execFileSync('git', args, { cwd: root, input, stdio: 'pipe', maxBuffer: 1 << 30 })
}

export function isLfsPointer(buffer) {
  return buffer.subarray(0, LFS_POINTER_PREFIX.length).toString('latin1') === LFS_POINTER_PREFIX
}

/**
 * Returns the check IDs that `buffer` fails. `allowedIds` are the Figma file ids allowlisted for
 * this path: a URL fails `url` unless its file id is one of them.
 */
export function scanBuffer(buffer, key, allowedIds = new Set()) {
  const text = buffer.toString('latin1')
  const failed = []
  for (const match of text.matchAll(URL_PATTERN))
    if (!allowedIds.has(match[1])) {
      failed.push('url')
      break
    }
  if (key && text.includes(key)) failed.push('key')
  return failed
}

/**
 * path -> Figma file ids allowed in that file for the `url` check. Each entry pins one public file
 * id in one path, so any other URL added to the same file still fails. Only `url` entries count:
 * the private key is never acceptable anywhere.
 */
export function loadAllowlist(root, rev) {
  let raw
  try {
    raw = rev
      ? git(root, ['show', `${rev}:${ALLOWLIST_PATH}`]).toString('utf8')
      : readFileSync(join(root, ALLOWLIST_PATH), 'utf8')
  } catch {
    return new Map()
  }
  const allowlist = new Map()
  for (const entry of JSON.parse(raw)) {
    if (entry.check !== 'url' || typeof entry.id !== 'string' || !entry.id) continue
    if (!allowlist.has(entry.path)) allowlist.set(entry.path, new Set())
    allowlist.get(entry.path).add(entry.id)
  }
  return allowlist
}

/** The oid and size a Git LFS pointer names, or null when it names none. */
export function parseLfsPointer(buffer) {
  const text = buffer.toString('utf8')
  const oid = /^oid sha256:([0-9a-f]{64})$/m.exec(text)?.[1]
  const size = /^size ([0-9]+)$/m.exec(text)?.[1]
  return oid && size !== undefined ? { oid, size: Number(size) } : null
}

const lfsStores = new Map()
/** The local LFS object store of `root`, located through Git alone: the LFS client is never run here. */
export function lfsStore(root) {
  if (!lfsStores.has(root)) {
    const common = git(root, ['rev-parse', '--path-format=absolute', '--git-common-dir'])
      .toString()
      .trim()
    let storage = ''
    try {
      storage = git(root, ['config', '--get', 'lfs.storage']).toString().trim()
    } catch {
      // unset: the default store under the common Git directory
    }
    lfsStores.set(
      root,
      storage ? resolve(common, storage, 'objects') : join(common, 'lfs', 'objects')
    )
  }
  return lfsStores.get(root)
}

function materializeLfs(root, pointer) {
  // Only the explicitly prefetched local store is read. Neither a historical pointer nor a missing object
  // may initiate a network request from the scan step, where credentials are deliberately absent.
  try {
    const parsed = parseLfsPointer(pointer)
    if (!parsed) return null
    const { oid, size } = parsed
    const store = lfsStore(root)
    const file = join(store, oid.slice(0, 2), oid.slice(2, 4), oid)
    if (statSync(file).size !== size) return null
    const content = readFileSync(file)
    return createHash('sha256').update(content).digest('hex') === oid ? content : null
  } catch {
    return null
  }
}

function scanContent(root, buffer, key, allowedIds, materialize = false) {
  if (!isLfsPointer(buffer)) return scanBuffer(buffer, key, allowedIds)
  // The pointer bytes are scanned too: a valid pointer line may be followed by any text.
  const pointer = scanBuffer(buffer, key, allowedIds)
  if (!materialize) return [...pointer, 'lfs']
  const content = materializeLfs(root, buffer)
  return content && !isLfsPointer(content)
    ? [...new Set([...pointer, ...scanBuffer(content, key, allowedIds)])]
    : [...pointer, 'lfs']
}

function scanWorktree(root, key, allowed, report) {
  const files = git(root, ['ls-files', '-z', '--cached', '--others', '--exclude-standard'])
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
  let scanned = 0
  for (const path of files) {
    let buffer
    try {
      if (!statSync(join(root, path)).isFile()) continue
      buffer = readFileSync(join(root, path))
    } catch {
      continue // deleted in the worktree, or a submodule directory
    }
    scanned += 1
    for (const check of scanContent(root, buffer, key, allowed(path))) report(check, path)
  }
  return scanned
}

// Every blob a commit of `range` adds or changes, merges included, with the commit and path it came from.
function rangeBlobs(root, range) {
  const log = git(root, [
    'log',
    '--root',
    '--raw',
    '--no-abbrev',
    '--diff-merges=cc',
    '-z',
    '--no-renames',
    '--format=commit %H',
    range
  ]).toString('utf8')
  const commits = new Set()
  const blobs = []
  let commit = ''
  const fields = log.split('\0')
  for (let i = 0; i < fields.length; i += 1) {
    const field = fields[i].replace(/^\n+/, '')
    if (field.startsWith('commit ')) {
      commit = field.slice('commit '.length).trim()
      commits.add(commit)
      continue
    }
    // Combined entries have one mode and hash per parent plus the result, and a multi-letter status.
    const match = /^(:+)(.+)$/.exec(field)
    if (!match) continue
    const parts = match[2].split(' ')
    const parents = match[1].length
    if (parts.length !== 2 * (parents + 1) + 1) continue
    const sha = parts.at(-2)
    if (!/^[0-9a-f]{40}$/.test(sha) || !/^[A-Z]+$/.test(parts.at(-1))) continue
    const path = fields[++i]
    if (/^0+$/.test(sha)) continue
    blobs.push({ commit, path, sha })
  }
  return { commits: commits.size, blobs }
}

// A pointer anywhere in the range is read from the prefetched local store (lfs-prefetch.mjs), whatever its
// path: a payload the store does not hold fails `lfs` rather than going unscanned.
function scanRange(root, range, key, allowed, report) {
  const { commits, blobs } = rangeBlobs(root, range)
  for (const { commit: sha, path, sha: blob } of blobs) {
    const buffer = git(root, ['cat-file', 'blob', blob])
    for (const check of scanContent(root, buffer, key, allowed(path), true))
      report(check, path, sha)
  }
  return { commits, blobs: blobs.length }
}

// Every blob under `prefix` at `rev`, read with cat-file: no checkout, no filters, no hooks.
function treeBlobs(root, rev, prefix) {
  const commit = git(root, ['rev-parse', '--verify', `${rev}^{commit}`])
    .toString()
    .trim()
  const blobs = git(root, ['ls-tree', '-r', '-z', '--full-tree', commit, '--', prefix])
    .toString('utf8')
    .split('\0')
    .flatMap((entry) => {
      const match = /^\d+ blob ([0-9a-f]+)\t(.*)$/s.exec(entry)
      return match ? [{ sha: match[1], path: match[2] }] : [] // a submodule commit: nothing to read
    })
  return { commit, blobs }
}

function scanTree(root, rev, prefix, key, allowed, report) {
  const { commit, blobs } = treeBlobs(root, rev, prefix)
  for (const { sha, path } of blobs) {
    const buffer = git(root, ['cat-file', 'blob', sha])
    for (const check of scanContent(root, buffer, key, allowed(path), true))
      report(check, path, commit)
  }
  return { commit, scanned: blobs.length }
}

/**
 * The distinct LFS objects the range and trees of a scan point to, as { oid, size }: the list lfs-prefetch.mjs
 * downloads before the scan. Nothing but object ids leaves here, never a path or a payload.
 */
export function listLfsObjects({ root, range, trees = [] }) {
  const shas = [
    ...(range ? rangeBlobs(root, range).blobs : []),
    ...trees.flatMap(({ rev, prefix }) => treeBlobs(root, rev, prefix).blobs)
  ].map((blob) => blob.sha)
  const objects = new Map()
  for (const sha of new Set(shas)) {
    const buffer = git(root, ['cat-file', 'blob', sha])
    const pointer = isLfsPointer(buffer) ? parseLfsPointer(buffer) : null
    if (pointer) objects.set(pointer.oid, pointer)
  }
  return [...objects.values()]
}

function scanPath(root, target, key, allowed, report) {
  const absolute = resolve(root, target)
  let scanned = 0
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) walk(full)
      else if (entry.isFile()) {
        scanned += 1
        const path = relative(root, full)
        for (const check of scanContent(root, readFileSync(full), key, allowed(path)))
          report(check, path)
      }
    }
  }
  if (statSync(absolute).isDirectory()) walk(absolute)
  else {
    scanned = 1
    const path = relative(root, absolute)
    for (const check of scanContent(root, readFileSync(absolute), key, allowed(path)))
      report(check, path)
  }
  return scanned
}

// An unpacked artifact is adversarial input. Its measured Review build is 2,332 files, 93 MiB, 3 MiB at most,
// 15 levels deep, so these limits leave room without letting a bomb through.
export const ARTIFACT_LIMITS = {
  files: 5000,
  totalBytes: 256 * 1024 * 1024,
  fileBytes: 16 * 1024 * 1024,
  depth: 32
}

/**
 * Lists the regular files under `directory` without following any link, and fails closed: a symlink, a hard
 * link, anything that resolves outside the directory, or a breach of a limit is a problem, never skipped.
 * Returns { files: [{ full, path }], problems: [{ path, reason }] }.
 */
export function enumerateArtifact(directory, limits = ARTIFACT_LIMITS) {
  const base = realpathSync(directory)
  const files = []
  const problems = []
  let total = 0
  const walk = (dir, depth) => {
    if (depth > limits.depth) {
      problems.push({ path: relative(base, dir), reason: 'too deep' })
      return
    }
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      const path = relative(base, full)
      const stat = lstatSync(full)
      if (stat.isSymbolicLink()) problems.push({ path, reason: 'symlink' })
      else if (stat.isDirectory()) {
        const real = realpathSync(full)
        if (real !== base && !real.startsWith(base + sep))
          problems.push({ path, reason: 'escapes the directory' })
        else walk(full, depth + 1)
      } else if (stat.isFile()) {
        if (stat.nlink > 1) problems.push({ path, reason: 'hard link' })
        else if (stat.size > limits.fileBytes) problems.push({ path, reason: 'file too large' })
        else if (files.length >= limits.files) {
          if (!problems.some((problem) => problem.reason === 'too many files'))
            problems.push({ path, reason: 'too many files' })
        } else if (total + stat.size > limits.totalBytes) {
          if (!problems.some((problem) => problem.reason === 'too large in total'))
            problems.push({ path, reason: 'too large in total' })
        } else {
          total += stat.size
          files.push({ full, path })
        }
      } else problems.push({ path, reason: 'not a regular file' })
    }
  }
  walk(base, 0)
  return { files, problems }
}

function scanArtifact(root, target, key, allowed, report, limits) {
  const { files, problems } = enumerateArtifact(resolve(root, target), limits)
  for (const { path, reason } of problems) report('artifact', `${path} (${reason})`)
  for (const { full, path } of files)
    for (const check of scanContent(root, readFileSync(full), key, allowed(path)))
      report(check, path)
  return files.length
}

function parseArgs(argv) {
  const options = {
    range: 'origin/main..HEAD',
    worktree: true,
    paths: [],
    trees: [],
    artifacts: [],
    allowlistRev: undefined,
    revision: undefined,
    root: undefined,
    lfsObjects: false
  }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--range') options.range = argv[++i]
    else if (arg === '--no-range') options.range = null
    else if (arg === '--no-worktree') options.worktree = false
    else if (arg === '--path') options.paths.push(argv[++i])
    else if (arg === '--root') options.root = argv[++i]
    else if (arg === '--artifact') options.artifacts.push(argv[++i])
    else if (arg === '--tree') {
      const value = argv[++i] ?? ''
      const colon = value.indexOf(':')
      if (colon <= 0 || colon === value.length - 1) throw new Error('--tree takes <rev>:<path>')
      options.trees.push({ rev: value.slice(0, colon), prefix: value.slice(colon + 1) })
    } else if (arg === '--allowlist-from') options.allowlistRev = argv[++i]
    else if (arg === '--revision') {
      // The commit this copy of the guard was read from, when it runs outside a checkout of that commit.
      options.revision = argv[++i]
      if (!options.revision || options.revision.startsWith('-'))
        throw new Error('--revision takes a commit')
    } else if (arg === '--lfs-objects') options.lfsObjects = true
    else throw new Error(`unknown argument: ${arg}`)
  }
  return options
}

/** Runs the guard and returns { findings, lines }. Never includes the key or a matched line. */
export function runGuard({
  root,
  range,
  worktree,
  paths,
  trees = [],
  artifacts = [],
  artifactLimits,
  allowlistRev,
  revision = 'HEAD',
  key
}) {
  const allowlist = loadAllowlist(root, allowlistRev)
  const findings = []
  const allowed = (path) => allowlist.get(path)
  const report = (check, path, commit) => {
    findings.push({
      check,
      location: redact(commit ? `${commit.slice(0, 12)}:${path}` : path, key)
    })
  }
  const lines = []
  const at = git(root, ['rev-parse', '--verify', `${revision}^{commit}`])
  lines.push(`figma-key-guard at ${at.toString().trim()}`)
  lines.push(
    `check url: ran (allowlisted public file ids in ${allowlist.size} paths${allowlistRev ? ` from ${allowlistRev}` : ''})`
  )
  lines.push(key ? 'check key: ran' : 'check key: SKIPPED (FIGMA_FILE_KEY not set)')
  if (worktree) lines.push(`worktree: ${scanWorktree(root, key, allowed, report)} files`)
  else lines.push('worktree: SKIPPED')
  if (range) {
    const resolved = range
      .split('..')
      .map((ref) => git(root, ['rev-parse', '--short=12', ref]).toString().trim())
      .join('..')
    const { commits, blobs } = scanRange(root, range, key, allowed, report)
    lines.push(`range: ${range} (${resolved}): ${commits} commits, ${blobs} blobs`)
  } else lines.push('range: SKIPPED')
  for (const { rev, prefix } of trees) {
    const { commit, scanned } = scanTree(root, rev, prefix, key, allowed, report)
    lines.push(`tree ${rev}:${prefix} (${commit.slice(0, 12)}): ${scanned} files`)
  }
  for (const target of artifacts)
    lines.push(
      `artifact ${target}: ${scanArtifact(root, target, key, allowed, report, artifactLimits)} files`
    )
  for (const target of paths)
    lines.push(`path ${target}: ${scanPath(root, target, key, allowed, report)} files`)
  for (const { check, location } of findings) lines.push(`FAIL ${check} ${location}`)
  lines.push(`result: ${findings.length === 0 ? 'PASS' : `FAIL (${findings.length} findings)`}`)
  return { findings, lines: lines.map((line) => redact(line, key)) }
}

/** Command-line entry point (also `dcl-figma key-guard`). Returns the exit code. */
export function main(argv) {
  const key = process.env.FIGMA_FILE_KEY || undefined
  try {
    const options = parseArgs(argv)
    const root =
      options.root ?? git(process.cwd(), ['rev-parse', '--show-toplevel']).toString().trim()
    // Lists, one JSON object per line, the LFS objects the same scan would read; scans nothing.
    if (options.lfsObjects) {
      const objects = listLfsObjects({ root, range: options.range, trees: options.trees })
      if (objects.length)
        output('stdout', objects.map((item) => JSON.stringify(item)).join('\n'), key)
      return 0
    }
    const { findings, lines } = runGuard({
      ...options,
      root,
      key
    })
    output('stdout', lines.join('\n'), key)
    return findings.length === 0 ? 0 : 1
  } catch (error) {
    // git errors can echo command input; print only the message's first line.
    output(
      'stderr',
      `figma-key-guard: ${String(error instanceof Error ? error.message : error).split('\n')[0]}`,
      key
    )
    return 2
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2))
}
