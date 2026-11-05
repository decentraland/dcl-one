#!/usr/bin/env node
// figma-key-guard: fails when the private Figma file key, or any Figma file URL, is present in
// the working tree, in a blob of the outgoing commit range, or in a generated output directory.
// Diagnostics name the path (or commit:path) and the check ID only, never the match or the key.
// Dependency-free so it runs before `pnpm install` and in fork CI.
import { execFileSync } from 'node:child_process'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const URL_PATTERN = /figma\.com\/(?:design|file|proto|board)\/([A-Za-z0-9]+)/g
const LFS_POINTER_PREFIX = 'version https://git-lfs'
const ALLOWLIST_PATH = 'library/ui/figma/key-guard-allow.json'

function git(root, args, input) {
  return execFileSync('git', args, { cwd: root, input, maxBuffer: 1 << 30 })
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
export function loadAllowlist(root) {
  let raw
  try {
    raw = readFileSync(join(root, ALLOWLIST_PATH), 'utf8')
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

function materializeLfs(root, pointer) {
  try {
    return git(root, ['lfs', 'smudge'], pointer)
  } catch {
    return null
  }
}

function scanContent(root, buffer, key, allowedIds) {
  if (!isLfsPointer(buffer)) return scanBuffer(buffer, key, allowedIds)
  const content = materializeLfs(root, buffer)
  return content && !isLfsPointer(content) ? scanBuffer(content, key, allowedIds) : ['lfs']
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

function scanRange(root, range, key, allowed, report) {
  const log = git(root, [
    'log',
    '--raw',
    '--no-abbrev',
    '-m',
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
    // Raw entry: ":<mode> <mode> <src> <dst> <status>" followed by the path field.
    const match = /^:\d+ \d+ [0-9a-f]+ ([0-9a-f]+) ([A-Z])/.exec(field)
    if (!match) continue
    const path = fields[i + 1]
    i += 1
    if (match[2] === 'D' || /^0+$/.test(match[1])) continue
    blobs.push({ commit, path, sha: match[1] })
  }
  for (const { commit: sha, path, sha: blob } of blobs) {
    const buffer = git(root, ['cat-file', 'blob', blob])
    for (const check of scanContent(root, buffer, key, allowed(path))) report(check, path, sha)
  }
  return { commits: commits.size, blobs: blobs.length }
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

function parseArgs(argv) {
  const options = { range: 'origin/main..HEAD', worktree: true, paths: [], root: undefined }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--range') options.range = argv[++i]
    else if (arg === '--no-range') options.range = null
    else if (arg === '--no-worktree') options.worktree = false
    else if (arg === '--path') options.paths.push(argv[++i])
    else if (arg === '--root') options.root = argv[++i]
    else throw new Error(`unknown argument: ${arg}`)
  }
  return options
}

/** Runs the guard and returns { findings, lines }. Never includes the key or a matched line. */
export function runGuard({ root, range, worktree, paths, key }) {
  const allowlist = loadAllowlist(root)
  const findings = []
  const allowed = (path) => allowlist.get(path)
  const report = (check, path, commit) => {
    findings.push({ check, location: commit ? `${commit.slice(0, 12)}:${path}` : path })
  }
  const lines = []
  lines.push(`figma-key-guard at ${git(root, ['rev-parse', 'HEAD']).toString().trim()}`)
  lines.push(`check url: ran (allowlisted public file ids in ${allowlist.size} paths)`)
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
  for (const target of paths)
    lines.push(`path ${target}: ${scanPath(root, target, key, allowed, report)} files`)
  for (const { check, location } of findings) lines.push(`FAIL ${check} ${location}`)
  lines.push(`result: ${findings.length === 0 ? 'PASS' : `FAIL (${findings.length} findings)`}`)
  return { findings, lines }
}

/** Command-line entry point (also `dcl-figma key-guard`). Returns the exit code. */
export function main(argv) {
  try {
    const options = parseArgs(argv)
    const root =
      options.root ?? execFileSync('git', ['rev-parse', '--show-toplevel']).toString().trim()
    const { findings, lines } = runGuard({
      ...options,
      root,
      key: process.env.FIGMA_FILE_KEY || undefined
    })
    console.log(lines.join('\n'))
    return findings.length === 0 ? 0 : 1
  } catch (error) {
    // git errors can echo command input; print only the message's first line.
    console.error(
      `figma-key-guard: ${String(error instanceof Error ? error.message : error).split('\n')[0]}`
    )
    return 2
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2))
}
