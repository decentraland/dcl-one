// The Figma capture this tooling reads lives in decentraland/figma-design-system (FDS), never in this repository.
// `fetchFdsSource` materializes the capture pinned in upstream.lock.json into the gitignored snapshot folder:
// it fetches exactly the pinned commit into a temporary repository, rewrites each capture file in memory into the
// keyless form this tree reads (no `fileKey`, Figma URLs replaced, embedded image metadata stripped, Windows-safe
// page file names, this tree's paths), and accepts the result only when every file matches the digest pinned for
// it in the lock. The raw FDS objects, which carry the file key, never reach the cache: the temporary repository
// is deleted before this returns. `verifyFdsCache` lets every consumer refuse a missing, stale or edited cache.
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { stripFileKey } from '../capture/strip-file-key.js'
import { encodePageFileName } from '../capture/page-file-name.js'
import { stripImageMetadata, stripSvgImageMetadata } from '../capture/strip-image-metadata.js'
import type { RawComponentPage } from '../ir/raw-components.js'
import { retirementsSchema } from '../guard/retirements.js'

const run = promisify(execFile)

/** The FDS folder that holds the capture, and the name this tree gives it. */
const FDS_SNAPSHOT = 'dcl-mcp-figma/figma-snapshot'
const LOCAL_SNAPSHOT = 'figma/figma-snapshot'
/** Written into the cache next to the capture; it names the revision and digest table the cache was built from. */
const MARKER = '.fds-source.json'
/**
 * dcl-one's own lifecycle ledgers, tracked here (figma/ledger). Each replaces figma-design-system's copy in the
 * cache, so the cache holds one effective ledger and a re-fetch never drops a record. The orphan ledger records
 * this tree's generated assets that a recapture stopped referencing, and may hold asset paths and hashes only.
 * The retirement ledger holds what `retire` writes: node ids, plain-text names and reasons, and artifact paths.
 */
const LEDGER = { tracked: 'ledger/orphaned-assets.json', cached: 'orphaned-assets.json' } as const
const RETIREMENT_LEDGER = {
  tracked: 'ledger/retirements.json',
  cached: 'retirements.json'
} as const

const parseJson = (bytes: Buffer, file: string): unknown => {
  try {
    return JSON.parse(bytes.toString('utf8'))
  } catch {
    throw new Error(`${file} is not valid JSON`)
  }
}

function parseRetirementLedger(bytes: Buffer): Buffer {
  if (!retirementsSchema.safeParse(parseJson(bytes, RETIREMENT_LEDGER.tracked)).success)
    throw new Error(
      `${RETIREMENT_LEDGER.tracked} may hold only { schema: 1, retired: [...] } with node ids, plain-text names and reasons, and repository paths`
    )
  return bytes
}

function parseLedger(bytes: Buffer): Buffer {
  const value = parseJson(bytes, LEDGER.tracked)
  const ledger = value as { schema?: unknown; orphaned?: unknown }
  const keys = value && typeof value === 'object' ? Object.keys(value) : []
  const entryOk = (entry: unknown) =>
    !!entry &&
    typeof entry === 'object' &&
    Object.keys(entry).length === 2 &&
    typeof (entry as { path?: unknown }).path === 'string' &&
    /^figma\/review\/src\/generated\/components\/assets\/[0-9a-f]+\.svg$/.test(
      (entry as { path: string }).path
    ) &&
    /^[0-9a-f]{64}$/.test(String((entry as { sha256?: unknown }).sha256))
  if (
    keys.length !== 2 ||
    ledger.schema !== 1 ||
    !Array.isArray(ledger.orphaned) ||
    !ledger.orphaned.every(entryOk)
  )
    throw new Error(`${LEDGER.tracked} may hold only { schema: 1, orphaned: [{ path, sha256 }] }`)
  return bytes
}

type FdsPin = {
  /** git URL the pinned commits are fetched from (SSH). */
  remote: string
  /** The code base the ported files are compared against (full 40-hex commit). */
  revision: string
  /** The commit the capture is read from (full 40-hex commit); it can be newer than the code base. */
  snapshotRevision: string
  /** Every cache file, relative to the snapshot folder, with the sha256 of its keyless content. */
  digests: Record<string, string>
}

/** The configured capture cache is missing, stale or edited: a broken checkout, never an absent capture. */
class FdsCacheError extends Error {
  override name = 'FdsCacheError'
}

/**
 * A capture path from the pinned tree, relative to the snapshot folder. It is joined into the staging folder, and
 * `--seal` takes the listing as the digest table, so a `..`, empty, `.`, absolute or backslash segment, or a path
 * that resolves outside the folder, is refused before anything is written.
 */
function assertCapturePath(file: string, root: string): void {
  const segments = file.split('/')
  const resolved = path.resolve(root, file)
  if (
    !file ||
    file.includes('\\') ||
    file.includes('\0') ||
    path.isAbsolute(file) ||
    path.win32.isAbsolute(file) ||
    segments.some((segment) => segment === '' || segment === '.' || segment === '..') ||
    !resolved.startsWith(root + path.sep)
  )
    throw new FdsCacheError('the pinned capture holds a path outside the snapshot folder')
}

const sha256 = (bytes: Buffer | string) => createHash('sha256').update(bytes).digest('hex')

/** Hash of a digest table, independent of key order. */
function digestTableHash(digests: Record<string, string>): string {
  return sha256(
    Object.keys(digests)
      .sort()
      .map((file) => `${file}\0${digests[file]}\n`)
      .join('')
  )
}

/** The only repository a lock may pin, so a lock edit cannot point fetch-source at a fork. */
const FDS_REPOSITORY = 'https://github.com/decentraland/figma-design-system'
const FDS_REMOTE = 'git@github.com:decentraland/figma-design-system.git'

/** Reads the FDS pin from upstream.lock.json. Throws unless it names a full commit and a non-empty digest table. */
function readFdsPin(lock: unknown, { sealing = false } = {}): FdsPin {
  const sources = (lock as { sources?: unknown })?.sources
  const source = Array.isArray(sources)
    ? (sources as Record<string, unknown>[]).find((entry) =>
        String(entry?.repository ?? '').endsWith('/figma-design-system')
      )
    : undefined
  if (!source) throw new Error('upstream.lock.json has no figma-design-system source')
  const revision = source.revision
  if (typeof revision !== 'string' || !/^[0-9a-f]{40}$/.test(revision))
    throw new Error('upstream.lock.json must pin figma-design-system to a full 40-hex commit')
  if (source.repository !== FDS_REPOSITORY)
    throw new Error(
      `upstream.lock.json must name the figma-design-system repository ${FDS_REPOSITORY}`
    )
  const snapshot = source.snapshot as { revision?: unknown; digests?: unknown } | undefined
  const snapshotRevision = snapshot?.revision
  if (typeof snapshotRevision !== 'string' || !/^[0-9a-f]{40}$/.test(snapshotRevision))
    throw new Error(
      'upstream.lock.json must pin the capture (snapshot.revision) to a full 40-hex commit'
    )
  const remote = FDS_REMOTE
  const digests = snapshot?.digests
  if (sealing) return { remote, revision, snapshotRevision, digests: {} }
  if (
    !digests ||
    typeof digests !== 'object' ||
    Array.isArray(digests) ||
    Object.keys(digests).length === 0 ||
    Object.entries(digests).some(
      ([file, digest]) =>
        typeof digest !== 'string' ||
        !/^[0-9a-f]{64}$/.test(digest) ||
        file.startsWith('/') ||
        file.split('/').includes('..')
    )
  )
    throw new Error('upstream.lock.json must carry a non-empty snapshot digest table')
  return { remote, revision, snapshotRevision, digests: digests as Record<string, string> }
}

const indentOf = (text: string) => /^[{[]\n( +)/.exec(text)?.[1]?.length

/**
 * Rewrites one FDS capture file into the keyless form this tree reads. `file` is relative to the FDS snapshot
 * folder; the result names its path relative to the local one.
 */
function keylessSnapshotFile(file: string, raw: Buffer): { file: string; bytes: Buffer } {
  const segments = file.split('/')
  const local =
    segments[0] === 'components' && segments.length === 2 && file.endsWith('.json')
      ? `components/${encodePageFileName(segments[1]!.slice(0, -'.json'.length))}`
      : file
  if (file.endsWith('.svg'))
    return { file: local, bytes: Buffer.from(stripSvgImageMetadata(stripFileKeyText(raw))) }
  if (!file.endsWith('.json')) return { file: local, bytes: Buffer.from(stripFileKeyText(raw)) }
  const text = raw.toString('utf8')
  let value: unknown = JSON.parse(text)
  if (segments[0] === 'components') value = stripImageMetadata(value as RawComponentPage)
  value = stripFileKey(value)
  const indent = indentOf(text)
  const serialized = (indent ? JSON.stringify(value, null, indent) : JSON.stringify(value))
    .split(`${FDS_SNAPSHOT}/`)
    .join(`${LOCAL_SNAPSHOT}/`)
  return { file: local, bytes: Buffer.from(serialized + (text.endsWith('\n') ? '\n' : '')) }
}

const stripFileKeyText = (raw: Buffer) => stripFileKey(raw.toString('utf8'))

const SCOPE = 'library/ui/figma/'

/** Where a file of this tree lives in figma-design-system, following figma/src/layout.ts. */
function fdsPathOf(file: string): string {
  const relative = file.slice(SCOPE.length)
  if (relative.startsWith('review/')) return `dcl-ui-review/${relative.slice('review/'.length)}`
  if (relative.startsWith('agent-kit/'))
    return `component-library/agent-kit/${relative.slice('agent-kit/'.length)}`
  return `dcl-mcp-figma/${relative}`
}

type Counterpart = { path: string; sha256: string } | null

const sameCounterpart = (a: unknown, b: Counterpart) =>
  a === null || b === null
    ? a === b
    : typeof a === 'object' &&
      (a as { path?: unknown }).path === b.path &&
      (a as { sha256?: unknown }).sha256 === b.sha256

type Git = (args: string[], options?: { input?: string }) => Promise<Buffer>

const defaultGit: Git = async (args, options) => {
  if (options?.input !== undefined) {
    return new Promise((resolve, reject) => {
      const child = execFile(
        'git',
        args,
        { encoding: 'buffer', maxBuffer: 1 << 30 },
        (error, stdout) => (error ? reject(error) : resolve(stdout))
      )
      child.stdin?.end(options.input)
    })
  }
  return (await run('git', args, { encoding: 'buffer', maxBuffer: 1 << 30 })).stdout
}

// Only the git verb and exit status reach an error: git's own message can echo object content.
function gitFailure(args: string[], error: unknown): Error {
  const code = (error as { code?: unknown })?.code
  const verb = (args[0] === '-C' ? args.slice(2) : args).find((arg) => !arg.startsWith('-')) ?? ''
  return new Error(`git ${verb} failed (${String(code ?? 'error')})`)
}

async function git(runner: Git, args: string[], options?: { input?: string }): Promise<Buffer> {
  try {
    return await runner(args, options)
  } catch (error) {
    throw gitFailure(args, error)
  }
}

/** Reads blobs by oid through one `git cat-file --batch`. */
function parseBatch(output: Buffer, oids: string[]): Map<string, Buffer> {
  const blobs = new Map<string, Buffer>()
  let offset = 0
  for (const oid of oids) {
    const end = output.indexOf(0x0a, offset)
    const header = output.subarray(offset, end).toString('utf8').split(' ')
    if (header[0] !== oid || header[1] !== 'blob')
      throw new Error('the pinned capture is incomplete')
    const size = Number(header[2])
    blobs.set(oid, output.subarray(end + 1, end + 1 + size))
    offset = end + 1 + size + 1
  }
  return blobs
}

type FetchOptions = {
  /** library/ui/figma */
  figmaRoot: string
  /** library/ui/upstream.lock.json */
  lockFile?: string
  /** Overrides the pinned remote; tests use a local repository. */
  remote?: string
  runner?: Git
  /** Overrides the directory rename of the cache swap; tests use it to fail one step. */
  rename?: (from: string, to: string) => Promise<void>
  /**
   * Porting only: accept whatever the pinned commit yields and write its digest table into the lock, instead of
   * checking it. The port verification record then has to vouch for the new table.
   */
  seal?: boolean
}

/**
 * Materializes the pinned capture into `<figmaRoot>/figma-snapshot`. Fails, leaving any previous cache in place,
 * when the commit cannot be fetched, the fetched commit is not the pinned one, or any resulting file is missing,
 * extra or differs from its pinned digest.
 */
async function fetchFdsSource(options: FetchOptions): Promise<{
  revision: string
  snapshotRevision: string
  files: number
  ported: number
  identical: number
}> {
  const runner = options.runner ?? defaultGit
  const lockFile = options.lockFile ?? path.resolve(options.figmaRoot, '../upstream.lock.json')
  const lock = JSON.parse(await readFile(lockFile, 'utf8')) as Record<string, unknown>
  const pin = readFdsPin(lock, { sealing: options.seal })
  const remote = options.remote ?? pin.remote
  const target = path.join(options.figmaRoot, 'figma-snapshot')
  const scratch = await mkdtemp(path.join(tmpdir(), 'fds-source-'))
  const staging = `${target}.staging-${process.pid}`
  try {
    const repo = path.join(scratch, 'repo.git')
    await git(runner, ['init', '--quiet', '--bare', repo])
    const inRepo = (args: string[], input?: string) =>
      git(runner, ['-C', repo, ...args], input === undefined ? undefined : { input })
    await inRepo(['remote', 'add', 'fds', remote])
    await inRepo(['config', 'remote.fds.promisor', 'true'])
    await inRepo(['config', 'remote.fds.partialclonefilter', 'blob:none'])
    await inRepo([
      'fetch',
      '--quiet',
      '--no-tags',
      '--depth',
      '1',
      '--filter=blob:none',
      'fds',
      pin.snapshotRevision
    ])
    const fetched = (await inRepo(['rev-parse', 'FETCH_HEAD^{commit}'])).toString().trim()
    if (fetched !== pin.snapshotRevision)
      throw new Error(
        `fetched ${fetched.slice(0, 12)}, but upstream.lock.json pins ${pin.snapshotRevision.slice(0, 12)}`
      )
    const listing = (
      await inRepo(['ls-tree', '-r', '-z', pin.snapshotRevision, '--', FDS_SNAPSHOT])
    )
      .toString('utf8')
      .split('\0')
      .filter(Boolean)
      .map((entry) => {
        const [meta, file] = entry.split('\t') as [string, string]
        const [, type, oid] = meta.split(' ') as [string, string, string]
        if (type !== 'blob') throw new Error('the pinned capture holds a non-file entry')
        if (!file.startsWith(`${FDS_SNAPSHOT}/`))
          throw new FdsCacheError('the pinned capture holds a path outside the snapshot folder')
        const relative = file.slice(FDS_SNAPSHOT.length + 1)
        assertCapturePath(relative, path.resolve(staging))
        return { file: relative, oid }
      })
    if (listing.length === 0) throw new Error('the pinned commit holds no capture')
    const oids = [...new Set(listing.map((entry) => entry.oid))]
    await inRepo(
      ['fetch', '--quiet', '--no-tags', '--filter=blob:none', '--stdin', 'fds'],
      oids.join('\n') + '\n'
    )
    const blobs = parseBatch(await inRepo(['cat-file', '--batch'], oids.join('\n') + '\n'), oids)

    const files = new Map<string, Buffer>()
    for (const { file, oid } of listing) {
      const result = keylessSnapshotFile(file, blobs.get(oid)!)
      assertCapturePath(result.file, path.resolve(staging))
      if (files.has(result.file)) throw new Error('two capture files map to the same local name')
      files.set(result.file, result.bytes)
    }
    files.set(
      LEDGER.cached,
      parseLedger(await readFile(path.join(options.figmaRoot, LEDGER.tracked)))
    )
    files.set(
      RETIREMENT_LEDGER.cached,
      parseRetirementLedger(await readFile(path.join(options.figmaRoot, RETIREMENT_LEDGER.tracked)))
    )
    const digests = options.seal
      ? Object.fromEntries(
          [...files]
            .sort(([a], [b]) => (a < b ? -1 : 1))
            .map(([file, bytes]) => [file, sha256(bytes)])
        )
      : pin.digests
    assertDigests(files, digests)

    // Every ported file's counterpart at the pinned commit: hashed in memory, never written anywhere.
    const source = (lock.sources as Record<string, unknown>[]).find(
      (entry) => entry.repository === FDS_REPOSITORY
    )!
    const ported =
      source.files && typeof source.files === 'object'
        ? (source.files as Record<string, { fds?: unknown }>)
        : {}
    if (pin.revision !== pin.snapshotRevision) {
      await inRepo([
        'fetch',
        '--quiet',
        '--no-tags',
        '--depth',
        '1',
        '--filter=blob:none',
        'fds',
        pin.revision
      ])
      const base = (await inRepo(['rev-parse', 'FETCH_HEAD^{commit}'])).toString().trim()
      if (base !== pin.revision)
        throw new Error(
          `fetched ${base.slice(0, 12)}, but upstream.lock.json pins the code base ${pin.revision.slice(0, 12)}`
        )
    }
    const tree = new Map(
      (
        await inRepo([
          'ls-tree',
          '-r',
          '-z',
          pin.revision,
          '--',
          'dcl-mcp-figma',
          'dcl-ui-review',
          'component-library/agent-kit'
        ])
      )
        .toString('utf8')
        .split('\0')
        .filter(Boolean)
        .map((entry) => {
          const [meta, file] = entry.split('\t') as [string, string]
          return [file, meta.split(' ')[2]!] as const
        })
    )
    const wanted = Object.keys(ported)
      .map((file) => ({ file, path: fdsPathOf(file) }))
      .filter(({ path: upstream }) => tree.has(upstream))
    const counterpartOids = [...new Set(wanted.map(({ path: upstream }) => tree.get(upstream)!))]
    const counterpartBlobs = counterpartOids.length
      ? (await inRepo(
          ['fetch', '--quiet', '--no-tags', '--filter=blob:none', '--stdin', 'fds'],
          counterpartOids.join('\n') + '\n'
        ),
        parseBatch(
          await inRepo(['cat-file', '--batch'], counterpartOids.join('\n') + '\n'),
          counterpartOids
        ))
      : new Map<string, Buffer>()
    const counterparts: Record<string, Counterpart> = Object.fromEntries(
      Object.keys(ported).map((file) => [file, null])
    )
    for (const { file, path: upstream } of wanted)
      counterparts[file] = {
        path: upstream,
        sha256: sha256(counterpartBlobs.get(tree.get(upstream)!)!)
      }
    if (!options.seal) {
      const stale = Object.keys(ported).filter(
        (file) => !sameCounterpart(ported[file]!.fds ?? null, counterparts[file]!)
      )
      if (stale.length)
        throw new Error(
          `${stale.length} ported files record another figma-design-system counterpart than the pinned commit holds (first: ${stale[0]})`
        )
    }

    await rm(staging, { recursive: true, force: true })
    for (const [file, bytes] of files) {
      await mkdir(path.dirname(path.join(staging, file)), { recursive: true })
      await writeFile(path.join(staging, file), bytes)
    }
    await writeFile(
      path.join(staging, MARKER),
      JSON.stringify(
        { revision: pin.snapshotRevision, digests: digestTableHash(digests) },
        null,
        2
      ) + '\n'
    )
    const move = options.rename ?? rename
    const previous = `${target}.previous-${process.pid}`
    const hadCache = existsSync(target)
    if (hadCache) await move(target, previous)
    try {
      await move(staging, target)
    } catch (error) {
      if (hadCache) await move(previous, target)
      throw error
    }
    await rm(previous, { recursive: true, force: true })
    if (options.seal) {
      source.snapshot = { revision: pin.snapshotRevision, path: FDS_SNAPSHOT, digests }
      for (const file of Object.keys(ported)) ported[file]!.fds = counterparts[file]
      await writeFile(lockFile, JSON.stringify(lock, null, 2) + '\n')
    }
    return {
      revision: pin.revision,
      snapshotRevision: pin.snapshotRevision,
      files: files.size,
      ported: Object.keys(ported).length,
      identical: Object.keys(ported).filter(
        (file) => counterparts[file]?.sha256 === (ported[file] as { sha256?: string }).sha256
      ).length
    }
  } finally {
    await rm(staging, { recursive: true, force: true })
    await rm(scratch, { recursive: true, force: true })
  }
}

function assertDigests(files: Map<string, Buffer>, digests: Record<string, string>): void {
  const missing = Object.keys(digests).filter((file) => !files.has(file))
  const extra = [...files.keys()].filter((file) => !(file in digests))
  const changed = [...files].filter(
    ([file, bytes]) => file in digests && sha256(bytes) !== digests[file]
  )
  const problems = [
    missing.length ? `${missing.length} missing` : '',
    extra.length ? `${extra.length} unexpected` : '',
    changed.length ? `${changed.length} differing from the pinned digest` : ''
  ].filter(Boolean)
  if (problems.length)
    throw new Error(
      `the capture does not match upstream.lock.json: ${problems.join(', ')} (first: ${[...missing, ...extra, ...changed.map(([file]) => file)][0]})`
    )
}

async function listFiles(root: string, prefix = ''): Promise<string[]> {
  const out: string[] = []
  for (const entry of await readdir(path.join(root, prefix), { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name
    if (entry.isDirectory()) out.push(...(await listFiles(root, relative)))
    else out.push(relative)
  }
  return out
}

/**
 * Accepts the cache in `dir` only when it was built from the pinned revision and digest table and no file was
 * added, removed or edited since. Every error names the command that repairs it.
 */
async function verifyFdsCache(dir: string, lockFile: string): Promise<void> {
  // A missing or malformed pin is a broken checkout too, never a reason to read the capture unchecked.
  let pin: FdsPin
  try {
    pin = readFdsPin(JSON.parse(await readFile(lockFile, 'utf8')))
  } catch (error) {
    throw new FdsCacheError(`The Figma capture pin is unusable: ${(error as Error).message}`, {
      cause: error
    })
  }
  const repair = 'run `pnpm figma fetch-source`'
  if (!existsSync(path.join(dir, MARKER)))
    throw new FdsCacheError(
      `The Figma capture is not materialized (${path.basename(dir)}): ${repair}`
    )
  let marker: { revision?: unknown; digests?: unknown }
  try {
    marker = JSON.parse(await readFile(path.join(dir, MARKER), 'utf8'))
  } catch (error) {
    throw new FdsCacheError(`The Figma capture marker is unreadable: ${repair}`, { cause: error })
  }
  // Valid JSON that is not an object (null, a number, an array) is a broken marker too.
  if (!marker || typeof marker !== 'object' || Array.isArray(marker))
    throw new FdsCacheError(`The Figma capture marker is malformed: ${repair}`)
  if (marker.revision !== pin.snapshotRevision || marker.digests !== digestTableHash(pin.digests))
    throw new FdsCacheError(
      `The Figma capture was built for another pin than upstream.lock.json: ${repair}`
    )
  const files = new Map<string, Buffer>()
  try {
    for (const file of await listFiles(dir)) {
      if (file === MARKER) continue
      files.set(file, await readFile(path.join(dir, file)))
    }
  } catch (error) {
    throw new FdsCacheError(`The Figma capture cannot be read: ${repair}`, { cause: error })
  }
  try {
    assertDigests(files, pin.digests)
  } catch (error) {
    throw new FdsCacheError(`${(error as Error).message}: ${repair}`, { cause: error })
  }
}

export {
  FdsCacheError,
  fdsPathOf,
  FDS_SNAPSHOT,
  MARKER,
  digestTableHash,
  fetchFdsSource,
  keylessSnapshotFile,
  readFdsPin,
  verifyFdsCache
}
export type { FdsPin, Git }
