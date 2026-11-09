#!/usr/bin/env node
// lock-digests: every tracked file under library/ui/figma is ported from decentraland/figma-design-system (FDS) or
// adapted from it, and upstream.lock.json pins the sha256 of each one. This check fails on a tracked file whose
// content differs from its pin, a pinned file that is no longer tracked, and a tracked file the lock does not pin,
// so nothing here changes without a lock change, and a lock change needs a port verification record
// (port-verification.mjs). Each file also records its FDS counterpart and, when it differs, the deviation codes that
// explain why. It reads the index, so it judges what would be committed; an LFS pointer fails.
// `--write` reseals the digests from the index and keeps counterparts and codes. Dependency-free; it needs no install and no capture.
import { Buffer } from 'node:buffer'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const SCOPE = 'library/ui/figma'
const LOCK = 'library/ui/upstream.lock.json'

const LFS_POINTER = Buffer.from('version https://git-lfs.github.com/spec/v1\n')
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

/** The FDS source entry of a parsed lock, or a thrown error. */
export function fdsSource(lock) {
  const source = Array.isArray(lock?.sources)
    ? lock.sources.find((entry) => String(entry?.repository ?? '').endsWith('/figma-design-system'))
    : undefined
  if (!source) throw new Error('upstream.lock.json has no figma-design-system source')
  return source
}

const DIGEST = /^[0-9a-f]{64}$/
const NOTE = /^[\w ,.;:()'/-]{1,160}$/

/**
 * Validates the pinned table and returns `{ digests, problems }`: `digests` maps each pinned path to its sha256,
 * and `problems` lists deviation errors. Each entry is { sha256, fds: { path, sha256 } | null, deviation?, note? }.
 * An entry whose content differs from its FDS blob, or that has no FDS counterpart, must name one or more codes of
 * `deviationCodes`, each linked to the changes[] entry that gives the reason; the code `note` instead carries a
 * short per-file `note`, for a difference no code explains. An absent, empty or malformed table is an error, never
 * a pass.
 */
export function pinnedFiles(lock) {
  const source = fdsSource(lock)
  const files = source.files
  if (
    !files ||
    typeof files !== 'object' ||
    Array.isArray(files) ||
    Object.keys(files).length === 0
  )
    throw new Error('upstream.lock.json pins no ported files')
  const problems = []
  // The two pins: the code base the files are compared against, and the capture fetch-source reads with its table.
  const COMMIT = /^[0-9a-f]{40}$/
  if (!COMMIT.test(source.revision ?? ''))
    problems.push('the code base (revision) is not a full 40-hex commit')
  const snapshot = source.snapshot
  if (!snapshot || typeof snapshot !== 'object')
    problems.push('the capture pin (snapshot) is missing')
  else {
    if (!COMMIT.test(snapshot.revision ?? ''))
      problems.push('the capture pin (snapshot.revision) is not a full 40-hex commit')
    const table = snapshot.digests
    if (
      !table ||
      typeof table !== 'object' ||
      Array.isArray(table) ||
      Object.keys(table).length === 0 ||
      Object.entries(table).some(
        ([file, digest]) =>
          typeof digest !== 'string' || !DIGEST.test(digest) || file.split('/').includes('..')
      )
    )
      problems.push('the capture pin has no well-formed snapshot digest table')
  }
  const codes = source.deviationCodes ?? {}
  const changes = Array.isArray(source.changes) ? source.changes : []
  for (const [code, index] of Object.entries(codes))
    if (code !== 'note' && (!Number.isInteger(index) || typeof changes[index] !== 'string'))
      problems.push(`deviation code ${code} links to no changes[] entry`)
  const digests = {}
  for (const [file, entry] of Object.entries(files)) {
    const fds = entry?.fds
    if (
      !file.startsWith(`${SCOPE}/`) ||
      file.split('/').includes('..') ||
      !entry ||
      typeof entry !== 'object' ||
      !DIGEST.test(entry.sha256 ?? '') ||
      (fds !== null &&
        (typeof fds !== 'object' || typeof fds.path !== 'string' || !DIGEST.test(fds.sha256 ?? '')))
    )
      throw new Error(`upstream.lock.json pins a malformed ported file entry: ${file}`)
    digests[file] = entry.sha256
    const deviation =
      entry.deviation === undefined
        ? []
        : [entry.deviation].flat().filter((code) => code !== undefined)
    const differs = fds === null || fds.sha256 !== entry.sha256
    if (differs && deviation.length === 0)
      problems.push(`${file} differs from figma-design-system and names no deviation`)
    if (!differs && deviation.length > 0)
      problems.push(`${file} matches figma-design-system but still names a deviation`)
    for (const code of deviation) {
      if (typeof code !== 'string' || !(code in codes))
        problems.push(`${file} names an unknown deviation code`)
      if (code === 'note' && !(typeof entry.note === 'string' && NOTE.test(entry.note)))
        problems.push(`${file} needs a short plain note for its deviation`)
    }
    if (entry.note !== undefined && !deviation.includes('note'))
      problems.push(`${file} carries a note without the note code`)
  }
  return { digests, problems }
}

/** sha256 of every file the index tracks under the scope, keyed by repository path. */
export function trackedDigests(root) {
  const listing = execFileSync('git', ['-C', root, 'ls-files', '-s', '-z', '--', SCOPE], {
    maxBuffer: 1 << 28
  })
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .map((entry) => {
      const [meta, file] = entry.split('\t')
      const [mode, oid] = meta.split(' ')
      if (mode === '160000') throw new Error(`${file} is a submodule`)
      return { file, oid }
    })
  if (listing.length === 0) return {}
  const output = execFileSync('git', ['-C', root, 'cat-file', '--batch'], {
    input: listing.map(({ oid }) => oid).join('\n') + '\n',
    maxBuffer: 1 << 30
  })
  const digests = {}
  let offset = 0
  for (const { file, oid } of listing) {
    const end = output.indexOf(0x0a, offset)
    const [found, , size] = output.subarray(offset, end).toString('utf8').split(' ')
    if (found !== oid) throw new Error('git returned an unexpected object')
    const start = end + 1
    const content = output.subarray(start, start + Number(size))
    // A pointer's text is not the file: pinning it would vouch for content nobody hashed.
    if (content.subarray(0, LFS_POINTER.length).equals(LFS_POINTER))
      throw new Error(`${file} is a Git LFS pointer; ported files are plain git`)
    digests[file] = sha256(content)
    offset = start + Number(size) + 1
  }
  return digests
}

/** Compares pinned and tracked tables; returns { mismatched, missing, unpinned } as sorted path lists. */
export function compare(pinned, tracked) {
  const sorted = (list) => list.sort()
  return {
    mismatched: sorted(
      Object.keys(tracked).filter((file) => file in pinned && pinned[file] !== tracked[file])
    ),
    missing: sorted(Object.keys(pinned).filter((file) => !(file in tracked))),
    unpinned: sorted(Object.keys(tracked).filter((file) => !(file in pinned)))
  }
}

/** Command-line entry point. Returns the exit code. */
export function main(argv, root = process.cwd()) {
  const write = argv.includes('--write')
  if (argv.some((arg) => arg !== '--write')) {
    process.stderr.write('Usage: lock-digests [--write]\n')
    return 2
  }
  const top = execFileSync('git', ['-C', root, 'rev-parse', '--show-toplevel']).toString().trim()
  const lockFile = resolve(top, LOCK)
  try {
    const lock = JSON.parse(readFileSync(lockFile, 'utf8'))
    const tracked = trackedDigests(top)
    if (write) {
      if (Object.keys(tracked).length === 0) throw new Error(`no tracked files under ${SCOPE}`)
      // Keeps each file's FDS counterpart and deviation; a new file starts with no counterpart and no code, so
      // the check fails until a port records why it differs (pnpm figma fetch-source --seal fills counterparts).
      const previous = fdsSource(lock).files ?? {}
      fdsSource(lock).files = Object.fromEntries(
        Object.keys(tracked)
          .sort()
          .map((file) => {
            const kept = previous[file] && typeof previous[file] === 'object' ? previous[file] : {}
            return [file, { ...kept, sha256: tracked[file], fds: kept.fds ?? null }]
          })
      )
      writeFileSync(lockFile, JSON.stringify(lock, null, 2) + '\n')
      process.stdout.write(`lock-digests: pinned ${Object.keys(tracked).length} files\n`)
      return 0
    }
    const { digests, problems } = pinnedFiles(lock)
    const { mismatched, missing, unpinned } = compare(digests, tracked)
    for (const file of mismatched) process.stdout.write(`CHANGED ${file}\n`)
    for (const file of missing) process.stdout.write(`MISSING ${file}\n`)
    for (const file of unpinned) process.stdout.write(`UNPINNED ${file}\n`)
    for (const problem of problems) process.stdout.write(`DEVIATION ${problem}\n`)
    const failed = mismatched.length + missing.length + unpinned.length + problems.length
    process.stdout.write(
      failed
        ? `lock-digests: ${failed} files differ from upstream.lock.json. A port updates the lock (node library/ui/figma/lock-digests.mjs --write) and adds its port verification record.\n`
        : `lock-digests: ${Object.keys(tracked).length} files match upstream.lock.json\n`
    )
    return failed ? 1 : 0
  } catch (error) {
    process.stderr.write(
      `lock-digests: ${error instanceof Error ? error.message : String(error)}\n`
    )
    return 1
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2))
}
