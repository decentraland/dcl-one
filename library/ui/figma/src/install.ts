// port:9c6dfbe6, promotion/promote.ts installation subset. Seal, manifest verification and release publication removed.
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  rename,
  rm,
  writeFile
} from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'
import type { App, SourceKind } from './app.js'
import { generateFoundations } from './generators/tokens.js'
import { generateIcons } from './generators/icons.js'
import { isMissing, resolveContained, toPosix } from './guard/fs-safe.js'
import { readFdsPin, verifyFdsCache } from './sources/fds-source.js'

const execute = promisify(execFile)
const digest = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex')
const POINTER = Buffer.from('version https://git-lfs.github.com/spec/v1\n')
type InstallOptions = { target: string; source?: SourceKind; outDir?: string }
type InstallResult = {
  target: string
  outDir: string
  changed: boolean
  files: { path: string; sha256: string }[]
}

async function tree(directory: string): Promise<Map<string, Buffer> | undefined> {
  const info = await lstat(directory).catch((error) => {
    if (isMissing(error)) return undefined
    throw error
  })
  if (!info) return undefined
  if (!info.isDirectory() || info.isSymbolicLink())
    throw new Error('Install target is not a safe directory')
  const files = new Map<string, Buffer>()
  for (const relative of await readdir(directory, { recursive: true })) {
    const file = await resolveContained(directory, relative)
    const entry = await lstat(file)
    if (entry.isDirectory()) continue
    if (!entry.isFile()) throw new Error('Install target contains a special file')
    files.set(toPosix(relative), await readFile(file))
  }
  return files
}

function same(a: ReadonlyMap<string, Buffer> | undefined, b: ReadonlyMap<string, Buffer>): boolean {
  return (
    a !== undefined &&
    a.size === b.size &&
    [...a].every(([file, bytes]) => bytes.equals(b.get(file) ?? Buffer.alloc(0)) && b.has(file))
  )
}

async function cleanTarget(
  root: string,
  target: string,
  outDir: string,
  current: Map<string, Buffer> | undefined,
  planned: Map<string, Buffer>
) {
  if (!current || same(current, planned)) return
  if (outDir !== path.join(root, target)) throw new Error('Install target is dirty')
  const status = (
    await execute('git', [
      '-C',
      root,
      'status',
      '--porcelain',
      '--untracked-files=all',
      '--',
      target
    ])
  ).stdout
  if (status.trim()) throw new Error('Install target is dirty')
}

/** One install boundary. Additional targets use this transaction rather than another CLI or generator. */
async function runInstall(app: App, options: InstallOptions): Promise<InstallResult> {
  if (options.target !== 'foundations') throw new Error('Unknown install target: ' + options.target)
  if (options.source !== undefined && options.source !== 'snapshot')
    throw new Error('install accepts only the verified cache, never rest')
  if (!app.config.snapshotLock)
    throw new Error('install requires snapshotLock and a verified cache')
  const root = path.resolve(app.cwd, '..')
  const relative = options.outDir
    ? toPosix(path.relative(root, path.resolve(app.cwd, options.outDir)))
    : 'foundations'
  if (
    relative !== 'foundations' &&
    !/^\.agent-work\/figma-install\/[a-zA-Z0-9_-]+\/foundations$/.test(relative)
  )
    throw new Error('Install output must be foundations or a named private foundations stage')
  const outDir = await resolveContained(root, relative)
  const snapshot = path.resolve(app.cwd, app.config.snapshotDir)
  const lockPath = path.resolve(app.cwd, app.config.snapshotLock)
  await verifyFdsCache(snapshot, lockPath)
  const lockBytes = await readFile(lockPath)
  const lock = JSON.parse(lockBytes.toString('utf8')) as {
    publicFoundations?: { files: Record<string, { sha256: string; origin: { kind: string } }> }
  }
  readFdsPin(lock)
  const source = app.createSource('snapshot')
  if (source.name !== 'snapshot') throw new Error('install requires a snapshot source')
  await source.ready?.()
  const [collections, textStyles, gradients, icons] = await Promise.all([
    source.getTokenCollections(),
    source.getTextStyles(),
    source.getGradients(),
    source.getIcons()
  ])
  const generated = [
    ...generateFoundations({
      collections,
      textStyles,
      gradients,
      excludedPrimitives: app.config.excludedPrimitives
    }),
    ...generateIcons(icons, {}).files
  ]
  const planned = new Map<string, Buffer>()
  for (const file of generated) {
    await resolveContained(outDir, file.path)
    if (planned.has(file.path)) throw new Error('Duplicate install output')
    planned.set(file.path, Buffer.from(file.content))
  }
  if (!planned.size) throw new Error('No foundation output was generated')
  const publicFiles = lock.publicFoundations?.files
  if (!publicFiles || !Object.keys(publicFiles).length)
    throw new Error('install requires pinned public foundation sources')
  // Static composition, declarations and licensed assets are maintained sources, never generator output.
  for (const [file, entry] of Object.entries(publicFiles)) {
    if (entry.origin.kind === 'generated') continue
    if (planned.has(file)) throw new Error('Generated output overlaps a maintained source')
    const bytes = await readFile(await resolveContained(path.join(root, 'foundations'), file))
    if (bytes.subarray(0, POINTER.length).equals(POINTER) || digest(bytes) !== entry.sha256)
      throw new Error('Unverified static foundation source: ' + file)
    planned.set(file, bytes)
  }
  const verifyInputs = async () => {
    await verifyFdsCache(snapshot, lockPath)
    if (!lockBytes.equals(await readFile(lockPath)))
      throw new Error('The source lock changed during install')
    for (const [file, entry] of Object.entries(publicFiles)) {
      if (
        entry.origin.kind !== 'generated' &&
        !planned
          .get(file)!
          .equals(await readFile(await resolveContained(path.join(root, 'foundations'), file)))
      )
        throw new Error('Static foundation source changed during install')
    }
  }
  return publishInstall(root, options.target, outDir, planned, verifyInputs)
}

async function publishInstall(
  root: string,
  target: string,
  outDir: string,
  planned: Map<string, Buffer>,
  verifyInputs: () => Promise<void>
): Promise<InstallResult> {
  const current = await tree(outDir)
  await cleanTarget(root, target, outDir, current, planned)
  const result = {
    target,
    outDir,
    changed: !same(current, planned),
    files: [...planned]
      .sort(([a], [b]) => a.localeCompare(b, 'en'))
      .map(([file, bytes]) => ({ path: file, sha256: digest(bytes) }))
  }
  await mkdir(path.dirname(outDir), { recursive: true })
  const mutex = outDir + '.install-lock'
  const handle = await open(mutex, 'wx').catch((error) => {
    throw new Error('Install target is busy or inaccessible', { cause: error })
  })
  let temporary: string | undefined
  let backup: string | undefined
  let published = false
  try {
    await verifyInputs()
    const latest = await tree(outDir)
    if ((current === undefined) !== (latest === undefined) || (current && !same(latest, current)))
      throw new Error('Install target changed concurrently')
    if (!result.changed) return result
    temporary = await mkdtemp(path.join(path.dirname(outDir), `.${target}-install-`))
    for (const [file, bytes] of planned) {
      const target = await resolveContained(temporary, file)
      await mkdir(path.dirname(target), { recursive: true })
      await writeFile(target, bytes)
    }
    await resolveContained(root, toPosix(path.relative(root, outDir)))
    await verifyInputs()
    const finalTarget = await tree(outDir)
    if (
      (latest === undefined) !== (finalTarget === undefined) ||
      (latest && !same(finalTarget, latest))
    )
      throw new Error('Install target changed concurrently')
    if (latest) {
      backup = temporary + '-previous'
      await rename(outDir, backup)
    }
    try {
      await rename(temporary, outDir)
    } catch (error) {
      if (backup) {
        await rename(backup, outDir)
        backup = undefined
      }
      throw error
    }
    published = true
    return result
  } finally {
    try {
      if (temporary) await rm(temporary, { recursive: true, force: true })
      if (published && backup) await rm(backup, { recursive: true, force: true })
    } finally {
      try {
        await handle.close()
      } finally {
        await rm(mutex, { force: true })
      }
    }
  }
}

export { runInstall }
export type { InstallOptions, InstallResult }
