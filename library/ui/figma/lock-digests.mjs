#!/usr/bin/env node
// lock-digests: every tracked file under library/ui/figma is ported from decentraland/figma-design-system (FDS) or
// adapted from it, and upstream.lock.json pins the sha256 of each one. This check fails on a tracked file whose
// content differs from its pin, a pinned file that is no longer tracked, and a tracked file the lock does not pin,
// so nothing here changes without a lock change, and a change to a port input needs a port verification record
// (port-verification.mjs). Each file also records its FDS counterpart and, when it differs, the deviation codes that
// explain why; each code in `deviationCodes` carries its own reason, keyed by a stable id in sorted order. It reads the index, so it judges what would be committed; an LFS pointer fails.
// `--write` reseals the digests from the index, keeps counterparts and codes, and sorts the codes. Dependency-free; it needs no install and no capture.
import { publicFoundationProblems } from './public-foundations.mjs'
import { publicComponentsProblems } from './public-components.mjs'
import { Buffer } from 'node:buffer'
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const SCOPE = 'library/ui/figma'
export const LOCK = 'library/ui/upstream.lock.json'

const LFS_POINTER = Buffer.from('version https://git-lfs.github.com/spec/v1\n')
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

/** The only repository a lock may pin, so a lock edit cannot point the port at a fork. */
const FDS_REPOSITORY = 'https://github.com/decentraland/figma-design-system'

/** The FDS source entry of a parsed lock, or a thrown error. */
export function fdsSource(lock) {
  const source = Array.isArray(lock?.sources)
    ? lock.sources.find((entry) => String(entry?.repository ?? '').endsWith('/figma-design-system'))
    : undefined
  if (!source) throw new Error('upstream.lock.json has no figma-design-system source')
  if (source.repository !== FDS_REPOSITORY)
    throw new Error(
      `upstream.lock.json must name the figma-design-system repository ${FDS_REPOSITORY}`
    )
  return source
}

const DIGEST = /^[0-9a-f]{64}$/
const NOTE = /^[\w ,.;:()'/-]{1,160}$/

/**
 * Validates the pinned table and returns `{ digests, problems }`: `digests` maps each pinned path to its sha256,
 * and `problems` lists deviation errors. Each entry is { sha256, fds: { path, sha256 } | null, deviation?, note? }.
 * An entry whose content differs from its FDS blob, or that has no FDS counterpart, must name one or more codes of
 * `deviationCodes`, each mapped to the reason it stands for; the code `note` (mapped to null) instead carries a short
 * per-file `note`, for a difference no code explains. Codes are kept in sorted order so that two branches adding
 * different codes rarely touch the same lines. An absent, empty or malformed table is an error, never
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
  if (typeof codes !== 'object' || Array.isArray(codes))
    throw new Error('upstream.lock.json has a malformed deviationCodes table')
  for (const [code, reason] of Object.entries(codes)) {
    if (code === 'note') {
      if (reason !== null)
        problems.push('deviation code note must map to null; each file carries its own note')
    } else if (typeof reason !== 'string' || reason.trim().length === 0)
      problems.push(`deviation code ${code} has no reason`)
    else if (reason.length > 4000)
      problems.push(`deviation code ${code} has a reason longer than 4000 characters`)
  }
  const codeNames = Object.keys(codes)
  // An integer-like key always sorts first in a JS object, so it could never stay in string order.
  for (const code of codeNames)
    if (/^(?:0|[1-9]\d*)$/.test(code))
      problems.push(`deviation code ${code} is integer-like; use a word id`)
  if (codeNames.some((code, index) => index > 0 && codeNames[index - 1] >= code))
    problems.push('deviation codes are not in sorted order (lock-digests --write sorts them)')
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
      if (typeof code !== 'string' || !Object.hasOwn(codes, code))
        problems.push(`${file} names an unknown deviation code`)
      if (code === 'note' && !(typeof entry.note === 'string' && NOTE.test(entry.note)))
        problems.push(`${file} needs a short plain note for its deviation`)
    }
    if (entry.note !== undefined && !deviation.includes('note'))
      problems.push(`${file} carries a note without the note code`)
  }
  return { digests, problems }
}

/**
 * sha256 of each blob in `listing` ({ file, oid } in any order), keyed by repository path. A Git LFS pointer throws: its
 * text is not the file, and pinning it would vouch for content nobody hashed.
 */
export function blobDigests(root, listing) {
  return digestsOf(readBlobs(root, listing))
}

/** sha256 of each blob's bytes, keyed like `blobs`. */
export function digestsOf(blobs) {
  return Object.fromEntries(Object.entries(blobs).map(([file, content]) => [file, sha256(content)]))
}

/** The bytes of each blob in `listing`, keyed by repository path, with the same refusals as blobDigests. */
export function readBlobs(root, listing) {
  if (listing.length === 0) return {}
  if (new Set(listing.map(({ file }) => file)).size !== listing.length)
    throw new Error('the listing names a path twice')
  const output = execFileSync('git', ['-C', root, 'cat-file', '--batch'], {
    input: listing.map(({ oid }) => oid).join('\n') + '\n',
    maxBuffer: 1 << 30
  })
  const blobs = {}
  let offset = 0
  for (const { file, oid } of listing) {
    const end = output.indexOf(0x0a, offset)
    const [found, , size] = output.subarray(offset, end).toString('utf8').split(' ')
    if (found !== oid) throw new Error('git returned an unexpected object')
    const start = end + 1
    const content = output.subarray(start, start + Number(size))
    if (content.subarray(0, LFS_POINTER.length).equals(LFS_POINTER))
      throw new Error(`${file} is a Git LFS pointer; ported files are plain git`)
    blobs[file] = content
    offset = start + Number(size) + 1
  }
  return blobs
}

/** Modes of the files a port may hold: regular and executable files. Symlinks and submodules are refused. */
const FILE_MODES = ['100644', '100755']

/**
 * Parses NUL-separated `git ls-files -s -z` or `git ls-tree -r -z` output into { file, oid }. Git does not quote
 * paths under -z, so the path is everything after the first tab, and a path with a control character is refused.
 */
export function parseListing(output, format) {
  return output
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .map((entry) => {
      const tab = entry.indexOf('\t')
      if (tab < 0) throw new Error('git returned an unexpected listing')
      const meta = entry.slice(0, tab).split(' ')
      const file = entry.slice(tab + 1)
      if (
        [...file].some(
          (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
        )
      )
        throw new Error(`${JSON.stringify(file)} has a control character in its path`)
      const [mode, oid] = format === 'ls-tree' ? [meta[0], meta[2]] : [meta[0], meta[1]]
      if (!FILE_MODES.includes(mode))
        throw new Error(`${file} has mode ${mode}; only regular files are allowed`)
      return { file, oid }
    })
}

/** sha256 of every file the index tracks under the scope, keyed by repository path. */
export function trackedDigests(root) {
  const output = execFileSync('git', ['-C', root, 'ls-files', '-s', '-z', '--', SCOPE], {
    maxBuffer: 1 << 28
  })
  return blobDigests(root, parseListing(output, 'ls-files'))
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
      const source = fdsSource(lock)
      if (source.deviationCodes && typeof source.deviationCodes === 'object')
        source.deviationCodes = Object.fromEntries(
          Object.entries(source.deviationCodes).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
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
    const publicProblems = [
      ...publicFoundationProblems(top, lock),
      ...publicComponentsProblems(top, lock)
    ]
    for (const problem of publicProblems) process.stdout.write(`PUBLIC ${problem}\n`)
    const failed =
      mismatched.length + missing.length + unpinned.length + problems.length + publicProblems.length
    process.stdout.write(
      failed
        ? `lock-digests: ${failed} files differ from upstream.lock.json. A port updates the lock (node library/ui/figma/lock-digests.mjs --write) and, when a port input changed, its port verification record (port-verification.mjs).\n`
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
