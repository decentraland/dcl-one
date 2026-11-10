// port:9c6dfbe6, promotion/promote.ts installation subset. Seal, manifest verification and release publication removed.
import { execFile, execFileSync } from 'node:child_process'
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
import { canonicalPath, isMissing, resolveContained, toPosix } from './guard/fs-safe.js'
import { readFdsPin, verifyFdsCache } from './sources/fds-source.js'
import { configSchema } from './config.js'
import { planComponents } from './install-components.js'
import { runtimeFiles } from './promotion/eligibility.js'
import {
  componentDependencyPaths,
  componentModulePaths,
  createComponentGeneration,
  isComponentModulePath
} from '../component-generation.mjs'

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
      '--ignored=matching',
      '--',
      target
    ])
  ).stdout
  if (status.trim()) throw new Error('Install target is dirty')
}

/** One install boundary. Additional targets use this transaction rather than another CLI or generator. */
async function runInstall(app: App, options: InstallOptions): Promise<InstallResult> {
  if (options.target !== 'foundations' && options.target !== 'components')
    throw new Error('Unknown install target: ' + options.target)
  if (options.source !== undefined && options.source !== 'snapshot')
    throw new Error('install accepts only the verified cache, never rest')
  if (!app.config.snapshotLock)
    throw new Error('install requires snapshotLock and a verified cache')
  const root = path.resolve(app.cwd, '..')
  const relative = options.outDir
    ? toPosix(path.relative(root, path.resolve(app.cwd, options.outDir)))
    : options.target
  if (
    relative !== options.target &&
    !new RegExp(`^\\.agent-work/figma-install/[a-zA-Z0-9_-]+/${options.target}$`).test(relative)
  )
    throw new Error(
      `Install output must be ${options.target} or a named private ${options.target} stage`
    )
  const outDir = await resolveContained(root, relative)
  const snapshot = path.resolve(app.cwd, app.config.snapshotDir)
  const lockPath = path.resolve(app.cwd, app.config.snapshotLock)
  const configFile = path.resolve(app.configFile ?? path.join(app.cwd, 'figma.config.json'))
  const readConfig = async () => readRegular(app.cwd, toPosix(path.relative(app.cwd, configFile)))
  const configBytes = await readConfig()
  const normalizedConfig = configurationIdentity(app.config)
  if (
    configurationIdentity(configSchema.parse(JSON.parse(configBytes.toString('utf8')))) !==
    normalizedConfig
  )
    throw new Error('The loaded configuration differs from its file')
  const lockBytes = await readFile(lockPath)
  const assertLock = async () => {
    if (!lockBytes.equals(await readFile(lockPath)))
      throw new Error('The source lock changed during install')
    if (
      !configBytes.equals(await readConfig()) ||
      configurationIdentity(app.config) !== normalizedConfig
    )
      throw new Error('The configuration changed during install')
  }
  const verifyCache = async () => {
    await verifyFdsCache(snapshot, lockPath)
    await assertLock()
  }
  await verifyCache()
  const lock = JSON.parse(lockBytes.toString('utf8')) as {
    publicFoundations?: { files: Record<string, { sha256: string; origin: { kind: string } }> }
    sources: {
      repository: string
      files: Record<string, { sha256: string }>
      [key: string]: unknown
    }[]
  }
  const pin = readFdsPin(lock)
  const source = app.createSource('snapshot')
  if (source.name !== 'snapshot') throw new Error('install requires a snapshot source')
  await source.ready?.()
  await assertLock()
  if (options.target === 'components')
    return installComponents(
      app,
      outDir,
      lock,
      source,
      configFile,
      configBytes,
      verifyCache,
      assertLock
    )
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
  const generatedPaths = new Set(planned.keys())
  const staticInputs = new Map<string, Buffer>()
  // Static composition, declarations and licensed assets are maintained sources, never generator output.
  for (const [file, entry] of Object.entries(publicFiles)) {
    if (entry.origin.kind === 'generated') continue
    if (planned.has(file)) throw new Error('Generated output overlaps a maintained source')
    const bytes = await readFile(await resolveContained(path.join(root, 'foundations'), file))
    if (
      bytes.subarray(0, POINTER.length).equals(POINTER) ||
      (file !== 'source.json' && digest(bytes) !== entry.sha256)
    )
      throw new Error('Unverified static foundation source: ' + file)
    staticInputs.set(file, bytes)
    planned.set(file, bytes)
  }
  if (staticInputs.has('source.json')) {
    const original = staticInputs.get('source.json')!
    const prefix = (await execute('git', ['-C', root, 'rev-parse', '--show-prefix'])).stdout.trim()
    const template = execFileSync('git', [
      '-C',
      root,
      'show',
      ':' + prefix + 'foundations/source.json'
    ])
    if (digest(template) !== publicFiles['source.json']!.sha256)
      throw new Error('Unverified provenance template')
    const regenerated = await regenerateSource(root, template, pin, planned, generatedPaths)
    if (digest(original) !== publicFiles['source.json']!.sha256 && !original.equals(regenerated))
      throw new Error('Unverified static foundation source: source.json')
    planned.set('source.json', regenerated)
  }
  const verifyInputs = async () => {
    await verifyCache()
    for (const [file, entry] of Object.entries(publicFiles)) {
      if (
        entry.origin.kind !== 'generated' &&
        !staticInputs
          .get(file)!
          .equals(await readFile(await resolveContained(path.join(root, 'foundations'), file)))
      )
        throw new Error('Static foundation source changed during install')
    }
  }
  return publishInstall(root, options.target, outDir, planned, verifyInputs)
}

function configurationIdentity(value: unknown): string {
  return JSON.stringify(value, (_key, item: unknown) =>
    item && typeof item === 'object' && !Array.isArray(item)
      ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b, 'en')))
      : item
  )
}

function assertInstallPath(file: string): void {
  if (
    !file ||
    /[\\<>:"|?*]/.test(file) ||
    [...file].some(
      (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
    ) ||
    file
      .split('/')
      .some(
        (part) =>
          !part ||
          part === '.' ||
          part === '..' ||
          /[. ]$/.test(part) ||
          /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)
      )
  )
    throw new Error('Unsafe install input or output path')
}

async function readRegular(root: string, relative: string): Promise<Buffer> {
  assertInstallPath(relative)
  const file = await resolveContained(root, relative)
  if (!(await lstat(file)).isFile())
    throw new Error('Install input is not a regular file: ' + relative)
  const bytes = await readFile(file)
  if (bytes.subarray(0, POINTER.length - 1).equals(POINTER.subarray(0, POINTER.length - 1)))
    throw new Error('Install input is an unmaterialized pointer: ' + relative)
  return bytes
}

type LockedInput = { path: string; sha256: string; indexBlobSha: string; bytes: Buffer }
type ComponentSource = {
  repository: string
  files: Record<string, { sha256: string }>
  [key: string]: unknown
}

async function lockedInputs(
  repository: string,
  paths: string[],
  source: ComponentSource,
  maintained = new Set<string>()
): Promise<Map<string, LockedInput>> {
  for (const file of paths) {
    assertInstallPath(file)
    if (!maintained.has(file) && !/^[a-f0-9]{64}$/.test(source.files?.[file]?.sha256 ?? ''))
      throw new Error('Missing locked component input: ' + file)
  }
  if (new Set(paths).size !== paths.length) throw new Error('Duplicate locked component input')
  const git = (args: string[], input?: string) =>
    execFileSync(
      'git',
      ['--no-replace-objects', '--literal-pathspecs', '-C', repository, ...args],
      { input, maxBuffer: 1 << 28 }
    )
  if (
    git(['status', '--porcelain', '--untracked-files=all', '--', ...paths])
      .toString()
      .trim()
  )
    throw new Error('Commit component source changes before installing their output')
  const listing = git(['ls-files', '-s', '-z', '--', ...paths])
    .toString()
    .split('\0')
    .filter(Boolean)
  const index = new Map<string, string>()
  for (const entry of listing) {
    const [meta, file] = entry.split('\t')
    const [mode, oid, stage] = meta!.split(' ')
    if (
      !file ||
      !paths.includes(file) ||
      stage !== '0' ||
      !['100644', '100755'].includes(mode!) ||
      index.has(file)
    )
      throw new Error('Unsafe or unmerged component input index')
    index.set(file, oid!)
  }
  for (const file of paths)
    if (!index.has(file)) throw new Error('Untracked component input: ' + file)
  const oids = [...index.values()]
  const batch = git(['cat-file', '--batch'], oids.join('\n') + '\n')
  const inputs = new Map<string, LockedInput>()
  let offset = 0
  for (const [file, oid] of index) {
    const end = batch.indexOf(0x0a, offset)
    const [found, kind, size] = batch.subarray(offset, end).toString().split(' ')
    const start = end + 1
    const finish = start + Number(size)
    if (
      end < 0 ||
      found !== oid ||
      kind !== 'blob' ||
      !/^\d+$/.test(size ?? '') ||
      batch[finish] !== 0x0a
    )
      throw new Error('Invalid component input blob')
    const bytes = await readRegular(repository, file)
    if (
      (!maintained.has(file) && digest(bytes) !== source.files[file]!.sha256) ||
      !bytes.equals(batch.subarray(start, finish))
    )
      throw new Error('Unverified component input: ' + file)
    inputs.set(file, { path: file, sha256: digest(bytes), indexBlobSha: oid, bytes })
    offset = finish + 1
  }
  return inputs
}

/** Physical overrides must be checked independently of the helper they can shadow under TSX. */
async function componentPhysicalInventory(
  repository: string
): Promise<{ files: string[]; executable: string[] }> {
  const prefix = 'library/ui/figma'
  const executable = (file: string) =>
    /\.(?:js|jsx|ts|tsx|mjs|mts|cjs|cts)$/i.test(file) && !/\.(?:test|spec)\.[^.]+$/.test(file)
  const files: string[] = []
  const implementations: string[] = []
  const add = (file: string) => {
    assertInstallPath(file)
    files.push(file)
    if (executable(file)) implementations.push(file)
  }
  const walk = async (relative: string) => {
    for (const entry of await readdir(await resolveContained(repository, relative), {
      withFileTypes: true
    })) {
      const file = relative + '/' + entry.name
      if (
        /\/(?:fixtures|test|__tests__)(?:\/|$)/.test(file) ||
        /\.(?:test|spec)\.[^.]+$/.test(file)
      )
        continue
      if (entry.isSymbolicLink()) throw new Error('Component source inventory contains a symlink')
      if (entry.isDirectory()) await walk(file)
      else if (!entry.isFile())
        throw new Error('Component source inventory contains a special file')
      else add(file)
    }
  }
  for (const entry of await readdir(await resolveContained(repository, prefix), {
    withFileTypes: true
  })) {
    const file = prefix + '/' + entry.name
    if (!executable(file)) continue
    if (entry.isSymbolicLink() || !entry.isFile())
      throw new Error('Component helper implementation is unsafe')
    add(file)
  }
  await walk(prefix + '/src')
  return { files, executable: implementations }
}

async function installComponents(
  app: App,
  outDir: string,
  lock: { sources: ComponentSource[] },
  source: ReturnType<App['createSource']>,
  configFile: string,
  configBytes: Buffer,
  verifyCache: () => Promise<void>,
  assertLock: () => Promise<void>
): Promise<InstallResult> {
  const root = path.resolve(app.cwd, '..')
  const repository = (
    await execute('git', ['-C', root, 'rev-parse', '--show-toplevel'])
  ).stdout.trim()
  const pinned = lock.sources.find(
    (entry) => entry.repository === 'https://github.com/decentraland/figma-design-system'
  )
  if (!pinned) throw new Error('Missing component source pin')
  const configPath = toPosix(path.relative(repository, await canonicalPath(configFile)))
  const modulePaths = componentModulePaths(pinned)
  const assertInventory = async () => {
    const listing = (
      await execute('git', [
        '--no-replace-objects',
        '--literal-pathspecs',
        '-C',
        repository,
        'ls-files',
        '-z',
        '--',
        'library/ui/figma'
      ])
    ).stdout
    const tracked = new Set(listing.split('\0').filter(Boolean))
    const physical = await componentPhysicalInventory(repository)
    for (const file of physical.executable) {
      if (!tracked.has(file))
        throw new Error('Untracked or ignored component implementation override: ' + file)
      if (!modulePaths.includes(file))
        throw new Error('Component implementation is absent from the locked read-set: ' + file)
    }
    const scoped = (file: string) =>
      file.startsWith('library/ui/figma/src/') || path.posix.dirname(file) === 'library/ui/figma'
    const physicalModules = physical.files.filter(isComponentModulePath).sort()
    const trackedPhysicalModules = [...tracked]
      .filter((file) => scoped(file) && isComponentModulePath(file))
      .sort()
    if (JSON.stringify(physicalModules) !== JSON.stringify(trackedPhysicalModules))
      throw new Error('Component physical source inventory differs from the committed source lock')
    const inventory = [...tracked].filter(isComponentModulePath).sort()
    if (JSON.stringify(inventory) !== JSON.stringify([...modulePaths].sort()))
      throw new Error('Component module inventory differs from the committed source lock')
  }
  await assertInventory()

  const dependencies = new Set<string>(componentDependencyPaths)
  const paths = [...new Set([...modulePaths, configPath, ...dependencies])]
  const initial = await lockedInputs(repository, paths, pinned, dependencies)
  const configInput = initial.get(configPath)!
  if (!configInput.bytes.equals(configBytes))
    throw new Error('The configuration changed during install')
  const registryFile = 'components/promotion.json'
  const registryBytes = await readRegular(root, registryFile)
  const registry = JSON.parse(registryBytes.toString('utf8')) as Parameters<
    typeof planComponents
  >[0]['registry']
  const runtime = new Map<string, Buffer>()
  for (const file of runtimeFiles) {
    const key = 'library/ui/figma/review/src/components/' + file
    if (!modulePaths.includes(key) || !initial.has(key))
      throw new Error('Missing locked runtime input: ' + file)
    runtime.set(file, Buffer.from(initial.get(key)!.bytes))
  }
  const plan = await planComponents({
    source,
    config: structuredClone(app.config),
    registry: structuredClone(registry),
    runtime
  })
  const planned = new Map([...plan.files].map(([file, bytes]) => [file, Buffer.from(bytes)]))
  if (!planned.size || planned.has('generation.json'))
    throw new Error('Invalid component install output')
  const normalized = new Set<string>()
  for (const [file, bytes] of planned) {
    if (bytes.subarray(0, POINTER.length - 1).equals(POINTER.subarray(0, POINTER.length - 1)))
      throw new Error('Component install output is an unmaterialized pointer')
    assertInstallPath(file)
    await resolveContained(outDir, file)
    const key = file.normalize('NFC').toLowerCase()
    if (normalized.has(key)) throw new Error('Colliding component install output paths')
    normalized.add(key)
  }
  const modules = modulePaths.map((file) => {
    const { path, sha256, indexBlobSha } = initial.get(file)!
    return { path, sha256, indexBlobSha }
  })
  const { path: configName, sha256, indexBlobSha } = configInput
  planned.set(
    'generation.json',
    Buffer.from(
      createComponentGeneration({
        source: pinned,
        modules,
        dependencies: [...dependencies].map((file) => {
          const { path, sha256, indexBlobSha } = initial.get(file)!
          return { path, sha256, indexBlobSha }
        }),
        config: { path: configName, sha256, indexBlobSha },
        files: planned
      })
    )
  )
  const verifyInputs = async () => {
    await verifyCache()
    if (!registryBytes.equals(await readRegular(root, registryFile)))
      throw new Error('The component registry changed during install')
    await assertInventory()
    const current = await lockedInputs(repository, paths, pinned, dependencies)
    for (const [file, original] of initial) {
      const latest = current.get(file)!
      if (latest.indexBlobSha !== original.indexBlobSha || !latest.bytes.equals(original.bytes))
        throw new Error('Component input changed during install: ' + file)
    }
    await assertLock()
  }
  await verifyInputs()
  return publishInstall(root, 'components', outDir, planned, verifyInputs)
}

type FoundationSource = {
  toolingCommit: string
  capture: { repository: string; revision: string; inputs: Record<string, string> }
  generatorModules: { path: string; sha256: string; blob: string }[]
  output: { path: string; sha256: string }[]
}

async function regenerateSource(
  root: string,
  original: Buffer,
  pin: ReturnType<typeof readFdsPin>,
  files: Map<string, Buffer>,
  generatedPaths: Set<string>
): Promise<Buffer> {
  const metadata = JSON.parse(original.toString('utf8')) as FoundationSource
  const repository = (
    await execute('git', ['-C', root, 'rev-parse', '--show-toplevel'])
  ).stdout.trim()
  const modules = []
  for (const module of metadata.generatorModules) {
    const bytes = await readFile(await resolveContained(repository, module.path))
    modules.push({
      path: module.path,
      sha256: digest(bytes),
      blob: execFileSync('git', ['-C', repository, 'hash-object', '--stdin'], {
        input: bytes,
        encoding: 'utf8'
      }).trim()
    })
  }
  if (JSON.stringify(modules) !== JSON.stringify(metadata.generatorModules)) {
    const paths = modules.map((module) => module.path)
    const commit = (
      await execute('git', ['-C', repository, 'log', '-1', '--format=%H', 'HEAD', '--', ...paths])
    ).stdout.trim()
    for (const module of modules) {
      const committed = execFileSync('git', ['-C', repository, 'show', `${commit}:${module.path}`])
      if (digest(committed) !== module.sha256)
        throw new Error('Commit generator changes before installing their output')
    }
    metadata.toolingCommit = commit
  }
  metadata.generatorModules = modules
  metadata.capture.revision = pin.snapshotRevision
  metadata.capture.inputs = Object.fromEntries(
    Object.keys(metadata.capture.inputs).map((file) => {
      if (!pin.digests[file]) throw new Error('Missing foundation capture input: ' + file)
      return [file, pin.digests[file]]
    })
  )
  metadata.output = [...files]
    .filter(([file]) => generatedPaths.has(file))
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([file, bytes]) => ({ path: file, sha256: digest(bytes) }))
  return Buffer.from(JSON.stringify(metadata, null, 2) + '\n')
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
        try {
          await rename(backup, outDir)
          backup = undefined
        } catch (rollbackError) {
          throw new AggregateError(
            [error, rollbackError],
            `Install publication failed, previous tree retained at ${backup}`,
            { cause: rollbackError }
          )
        }
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
