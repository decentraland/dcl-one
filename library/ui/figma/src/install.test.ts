import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  symlink,
  writeFile
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, type App } from './app.js'
import { verifyFdsCache } from './sources/fds-source.js'
import { runInstall } from './install.js'
import { copySnapshot, pinTestCapture, testConfig } from '../test/snapshot.js'
import {
  componentModulePaths,
  createComponentGeneration,
  isComponentModulePath
} from '../component-generation.mjs'
import { planComponents } from './install-components.js'
import { runtimeFiles } from './promotion/eligibility.js'
import { fixtureCommit, fixtureGit, initializeFixtureGit } from './guard/git-fixture.js'

vi.mock('node:fs/promises', async (original) => {
  const actual = await original<typeof import('node:fs/promises')>()
  return { ...actual, rename: vi.fn(actual.rename) }
})

vi.mock('./sources/fds-source.js', async (original) => {
  const actual = await original<typeof import('./sources/fds-source.js')>()
  return { ...actual, verifyFdsCache: vi.fn(actual.verifyFdsCache) }
})

vi.mock('../component-generation.mjs', async (original) => {
  const actual = await original<typeof import('../component-generation.mjs')>()
  return {
    ...actual,
    componentModulePaths: vi.fn(actual.componentModulePaths),
    isComponentModulePath: vi.fn(actual.isComponentModulePath),
    createComponentGeneration: vi.fn(actual.createComponentGeneration)
  }
})

vi.mock('./install-components.js', async (original) => {
  const actual = await original<typeof import('./install-components.js')>()
  return { ...actual, planComponents: vi.fn(actual.planComponents) }
})

describe('when foundations are installed from a verified synthetic cache', () => {
  let root: string
  let figma: string
  let app: App
  let lockPath: string
  let baseline: string[]
  const entry = '@layer dcl.reset, dcl.foundations, dcl.components;\n'

  beforeEach(async () => {
    vi.mocked(verifyFdsCache)
      .mockReset()
      .mockImplementation(
        (await vi.importActual<typeof import('./sources/fds-source.js')>('./sources/fds-source.js'))
          .verifyFdsCache
      )
    vi.mocked(rename).mockImplementation(
      (await vi.importActual<typeof import('node:fs/promises')>('node:fs/promises')).rename
    )
    root = await mkdtemp(path.join(tmpdir(), 'foundations-install-'))
    figma = path.join(root, 'figma')
    await mkdir(figma)
    await copySnapshot(path.join(figma, 'figma-snapshot'))
    await writeFile(
      path.join(figma, 'figma.config.json'),
      JSON.stringify({ ...(await testConfig()), snapshotDir: 'figma-snapshot' })
    )
    await pinTestCapture(figma)
    lockPath = path.join(figma, 'test-capture.lock.json')
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    await mkdir(path.join(figma, 'src/generators'), { recursive: true })
    await writeFile(path.join(figma, 'src/generators/demo.ts'), 'export const fixture = true\n')
    const metadata = JSON.stringify({
      schema: 1,
      toolingCommit: 'a'.repeat(40),
      capture: {
        repository: 'https://github.com/decentraland/figma-design-system',
        revision: 'a'.repeat(40),
        inputs: { 'variables.json': '0'.repeat(64) }
      },
      generatorModules: [
        { path: 'figma/src/generators/demo.ts', sha256: '0'.repeat(64), blob: '0'.repeat(40) }
      ],
      families: {},
      output: []
    })
    lock.publicFoundations = {
      files: {
        'foundations.css': {
          sha256: createHash('sha256').update(entry).digest('hex'),
          origin: { kind: 'source' }
        },
        'source.json': {
          sha256: createHash('sha256').update(metadata).digest('hex'),
          origin: { kind: 'source' }
        }
      }
    }
    await writeFile(lockPath, JSON.stringify(lock))
    await mkdir(path.join(root, 'foundations'))
    await writeFile(path.join(root, 'foundations/foundations.css'), entry)
    await writeFile(path.join(root, 'foundations/source.json'), metadata)
    await symlink(path.resolve('node_modules'), path.join(root, 'node_modules'), 'dir')
    await writeFile(path.join(root, '.gitignore'), 'node_modules/\n')
    for (const file of ['package.json', 'tsconfig.json', 'figma/tsconfig.json'])
      await writeFile(
        path.join(root, file),
        file === 'package.json' ? '{"name":"fixture-ui"}' : '{"compilerOptions":{}}'
      )
    await writeFile(path.join(root, 'pnpm-lock.yaml'), 'lockfileVersion: 9')
    lock.sources[0].files = {
      'figma/tsconfig.json': {
        sha256: createHash('sha256')
          .update(await readFile(path.join(figma, 'tsconfig.json')))
          .digest('hex')
      }
    }
    await writeFile(lockPath, JSON.stringify(lock))
    await initializeFixtureGit(root)
    app = await createApp(figma)
    baseline = await readdir(path.join(root, 'foundations'))
  })

  afterEach(async () => {
    vi.restoreAllMocks()
    await rm(root, { recursive: true, force: true })
  })

  it('should install through the CLI in its own process with the same result and exit code', async () => {
    const cli = async () =>
      JSON.parse(
        (
          await promisify(execFile)(
            process.execPath,
            [
              '--import',
              import.meta.resolve('tsx'),
              path.join(import.meta.dirname, 'cli.ts'),
              'install',
              'foundations'
            ],
            { cwd: figma }
          )
        ).stdout
      ) as Awaited<ReturnType<typeof runInstall>>
    const first = await cli()
    const second = await cli()
    expect(first.changed).toBe(true)
    expect(second.changed).toBe(false)
    expect(second.files).toEqual(first.files)
    expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe(entry)
  }, 60_000)

  it('should preserve maintained files and give identical bytes on a second install', async () => {
    const first = await runInstall(app, { target: 'foundations' })
    const second = await runInstall(app, { target: 'foundations' })
    expect(first.changed).toBe(true)
    expect(second.changed).toBe(false)
    expect(second.files).toEqual(first.files)
    expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe(entry)
    expect(first.files.some((file) => file.path.startsWith('icons/'))).toBe(true)
    expect(await readdir(root)).not.toContain('foundations.install-lock')
  })

  it('should reject a foundation dependency package scope without changing the prior tree', async () => {
    const original = await readFile(path.join(root, 'foundations/source.json'))
    await mkdir(path.join(figma, 'audit-dependency'), { recursive: true })
    await writeFile(
      path.join(figma, 'audit-dependency/svgo.mjs'),
      'export const optimize = () => ({data:"injected"})'
    )
    await writeFile(
      path.join(figma, 'package.json'),
      '{"name":"svgo","exports":"./audit-dependency/svgo.mjs"}'
    )
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow(
      'Unexpected component resolver scope'
    )
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
    expect(await readFile(path.join(root, 'foundations/source.json'))).toEqual(original)
    expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe(entry)
    expect(await readdir(root)).not.toContain('foundations.install-lock')
  })

  it('should reject a foundation resolver scope introduced during planning without publication', async () => {
    const source = app.createSource('snapshot')
    const getIcons = source.getIcons.bind(source)
    source.getIcons = async () => {
      const icons = await getIcons()
      await writeFile(
        path.join(figma, 'src/package.json'),
        '{"name":"svgo","exports":"./injected.mjs"}'
      )
      return icons
    }
    app.createSource = () => source
    const original = await readFile(path.join(root, 'foundations/source.json'))
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow(
      'Unexpected component resolver scope'
    )
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
    expect(await readFile(path.join(root, 'foundations/source.json'))).toEqual(original)
    expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe(entry)
  })

  it.each(['package.json', 'tsconfig.json', 'figma/tsconfig.json'])(
    'should reject dirty known foundation resolver input %s before publication',
    async (file) => {
      await writeFile(
        path.join(root, file),
        '{"compilerOptions":{"paths":{"svgo":["./injected.ts"]}}}'
      )
      await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow(
        'Commit component source changes'
      )
      expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
      expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe(entry)
    }
  )

  it('should reproduce a clean private stage without changing the public target', async () => {
    const options = {
      target: 'foundations',
      outDir: '../.agent-work/figma-install/proof/foundations'
    }
    const first = await runInstall(app, options)
    expect(first.changed).toBe(true)
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
    const second = await runInstall(app, options)
    expect(second.changed).toBe(false)
    expect(second.files).toEqual(first.files)
  })

  it.each(['', 'unknown', '../foundations'])(
    'should refuse the unknown target %s before writing',
    async (target) => {
      await expect(runInstall(app, { target })).rejects.toThrow('Unknown install target')
      expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
    }
  )

  it('should regenerate capture, output and generator provenance from committed inputs', async () => {
    const result = await runInstall(app, { target: 'foundations' })
    const metadata = JSON.parse(await readFile(path.join(root, 'foundations/source.json'), 'utf8'))
    expect(metadata.capture.revision).toBe('c'.repeat(40))
    expect(metadata.capture.inputs['variables.json']).not.toBe('0'.repeat(64))
    expect(metadata.generatorModules[0].sha256).toBe(
      createHash('sha256').update('export const fixture = true\n').digest('hex')
    )
    expect(metadata.output).toEqual(
      result.files.filter((file) => file.path !== 'source.json' && file.path !== 'foundations.css')
    )
  })

  it('should reject an uncommitted generator edit before writing provenance', async () => {
    await writeFile(path.join(figma, 'src/generators/demo.ts'), 'changed')
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow(
      'Commit generator changes'
    )
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should reject a provenance template that does not match its pin', async () => {
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    lock.publicFoundations.files['source.json'].sha256 = '0'.repeat(64)
    await writeFile(lockPath, JSON.stringify(lock))
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow(
      'Unverified provenance template'
    )
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should reject hand-edited provenance instead of accepting a self-consistent template', async () => {
    const file = path.join(root, 'foundations/source.json')
    const metadata = JSON.parse(await readFile(file, 'utf8'))
    metadata.families = { unexpected: {} }
    await writeFile(file, JSON.stringify(metadata))
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow(
      'Unverified static foundation source: source.json'
    )
    expect(JSON.parse(await readFile(file, 'utf8')).families).toEqual({ unexpected: {} })
  })

  it('should refuse the live source without writing', async () => {
    await expect(runInstall(app, { target: 'foundations', source: 'rest' })).rejects.toThrow(
      'never rest'
    )
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should refuse dirty output without overwriting it', async () => {
    await writeFile(path.join(root, 'foundations/untracked.ts'), 'keep')
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('dirty')
    expect(await readFile(path.join(root, 'foundations/untracked.ts'), 'utf8')).toBe('keep')
  })

  it('should refuse an edited maintained source', async () => {
    await writeFile(path.join(root, 'foundations/foundations.css'), 'keep')
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('Unverified static')
    expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe('keep')
  })

  it('should refuse a missing marker even through the synthetic bypass App', async () => {
    await rm(path.join(figma, 'figma-snapshot/.fds-source.json'))
    app = await createApp(figma, undefined, { unverifiedSnapshot: true })
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('not materialized')
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should bind the lock before the first verification and reject replacement immediately', async () => {
    const actual =
      await vi.importActual<typeof import('./sources/fds-source.js')>('./sources/fds-source.js')
    vi.mocked(verifyFdsCache).mockImplementationOnce(async (directory, file) => {
      await actual.verifyFdsCache(directory, file)
      const value = JSON.parse(await readFile(file, 'utf8'))
      value.replaced = true
      await writeFile(file, JSON.stringify(value))
    })
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('source lock changed')
    expect(verifyFdsCache).toHaveBeenCalledTimes(1)
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should reject a tampered cache before any read through a bypass App', async () => {
    await writeFile(path.join(figma, 'figma-snapshot/variables.json'), '[]')
    app = await createApp(figma, undefined, { unverifiedSnapshot: true })
    const source = app.createSource('snapshot')
    const read = vi.spyOn(source, 'getTokenCollections')
    app.createSource = () => source
    await expect(
      runInstall(app, {
        target: 'foundations',
        outDir: '../.agent-work/figma-install/rejected/foundations'
      })
    ).rejects.toThrow()
    expect(read).not.toHaveBeenCalled()
    await expect(readdir(path.join(root, '.agent-work'))).rejects.toThrow()
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should preserve ignored user files in a dirty target', async () => {
    await writeFile(path.join(root, '.gitignore'), '*.local\n')
    await writeFile(path.join(root, 'foundations/user.local'), 'synthetic user data')
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('dirty')
    expect(await readFile(path.join(root, 'foundations/user.local'), 'utf8')).toBe(
      'synthetic user data'
    )
  })

  it('should refuse an altered cache', async () => {
    await writeFile(path.join(figma, 'figma-snapshot/variables.json'), '{}')
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow()
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should refuse an absent snapshot lock', async () => {
    delete app.config.snapshotLock
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('snapshotLock')
  })

  it.each(['../figma/review', '../../outside', '../.agent-work/figma-install/../foundations'])(
    'should refuse the unowned output %s',
    async (outDir) => {
      await expect(runInstall(app, { target: 'foundations', outDir })).rejects.toThrow(
        'output must be'
      )
    }
  )

  it('should refuse a symlink target without following it', async () => {
    await mkdir(path.join(root, '.agent-work/figma-install/proof'), { recursive: true })
    await symlink(
      path.join(root, 'foundations'),
      path.join(root, '.agent-work/figma-install/proof/foundations')
    )
    await expect(
      runInstall(app, {
        target: 'foundations',
        outDir: '../.agent-work/figma-install/proof/foundations'
      })
    ).rejects.toThrow('symlink')
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })

  it('should refuse a busy target without removing another lock', async () => {
    await writeFile(path.join(root, 'foundations.install-lock'), 'other installer')
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('busy')
    expect(await readFile(path.join(root, 'foundations.install-lock'), 'utf8')).toBe(
      'other installer'
    )
  })

  it('should restore the previous directory if publication fails', async () => {
    const actual = await vi.importActual<typeof import('node:fs/promises')>('node:fs/promises')
    vi.mocked(rename)
      .mockImplementationOnce(actual.rename)
      .mockRejectedValueOnce(new Error('publication failed'))
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('publication failed')
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
    expect(await readFile(path.join(root, 'foundations/foundations.css'), 'utf8')).toBe(entry)
    expect(
      (await readdir(root)).filter((file) => file.startsWith('.foundations-install-'))
    ).toEqual([])
    expect((await runInstall(app, { target: 'foundations' })).changed).toBe(true)
  })

  it('should retain the previous directory if rollback also fails', async () => {
    const actual = await vi.importActual<typeof import('node:fs/promises')>('node:fs/promises')
    vi.mocked(rename)
      .mockImplementationOnce(actual.rename)
      .mockRejectedValueOnce(new Error('publication failed'))
      .mockRejectedValueOnce(new Error('rollback failed'))
    await expect(runInstall(app, { target: 'foundations' })).rejects.toMatchObject({
      name: 'AggregateError',
      message: expect.stringContaining('previous tree retained at'),
      errors: [
        expect.objectContaining({ message: 'publication failed' }),
        expect.objectContaining({ message: 'rollback failed' })
      ]
    })
    const backups = (await readdir(root)).filter((file) => file.endsWith('-previous'))
    expect(backups).toHaveLength(1)
    expect(await readFile(path.join(root, backups[0]!, 'foundations.css'), 'utf8')).toBe(entry)
    expect(await readdir(root)).not.toContain('foundations.install-lock')
  })

  it('should refuse a lock that changes during generation', async () => {
    const source = app.createSource('snapshot')
    const original = source.getIcons.bind(source)
    vi.spyOn(source, 'getIcons').mockImplementation(async () => {
      const result = await original()
      const lock = JSON.parse(await readFile(lockPath, 'utf8'))
      lock.changed = true
      await writeFile(lockPath, JSON.stringify(lock))
      return result
    })
    app.createSource = () => source
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('source lock changed')
    expect(await readdir(path.join(root, 'foundations'))).toEqual(baseline)
  })
})

describe('when component installation owns its verified input read-set and transaction', () => {
  let repository: string
  let root: string
  let figma: string
  let app: App
  let lockPath: string
  let configPath: string
  let modulePaths: string[]
  let registryPath: string
  let options: { target: string; outDir?: string }
  let originalRegistry: Buffer
  let actualCache: typeof verifyFdsCache
  let actualPlan: typeof planComponents
  let actualRename: typeof rename
  const hash = (bytes: Buffer | string) => createHash('sha256').update(bytes).digest('hex')
  const modulePath = 'library/ui/figma/src/fixture-generator.ts'
  const ledgerPath = 'library/ui/figma/ledger/retirements.json'
  const resolverPath = 'library/ui/figma/tsconfig.json'
  const stage = '../.agent-work/figma-install/components-proof/components'

  beforeEach(async () => {
    actualCache = (
      await vi.importActual<typeof import('./sources/fds-source.js')>('./sources/fds-source.js')
    ).verifyFdsCache
    actualPlan = (
      await vi.importActual<typeof import('./install-components.js')>('./install-components.js')
    ).planComponents
    actualRename = (await vi.importActual<typeof import('node:fs/promises')>('node:fs/promises'))
      .rename
    vi.mocked(verifyFdsCache).mockReset().mockImplementation(actualCache)
    vi.mocked(planComponents).mockReset().mockImplementation(actualPlan)
    vi.mocked(rename).mockReset().mockImplementation(actualRename)
    vi.mocked(createComponentGeneration)
      .mockReset()
      .mockImplementation(({ files, modules, config, dependencies }) =>
        Buffer.from(
          JSON.stringify({
            modules,
            config,
            dependencies,
            output: [...files].map(([name, bytes]) => ({ path: name, sha256: hash(bytes) }))
          }) + '\n'
        )
      )
    repository = await mkdtemp(path.join(tmpdir(), 'components-install-'))
    root = path.join(repository, 'library/ui')
    figma = path.join(root, 'figma')
    await mkdir(path.join(figma, 'src'), { recursive: true })
    await copySnapshot(path.join(figma, 'figma-snapshot'))
    configPath = path.join(figma, 'figma.config.json')
    await writeFile(
      configPath,
      JSON.stringify({
        ...(await testConfig()),
        snapshotDir: 'figma-snapshot',
        imageFills: 'placeholder'
      })
    )
    await pinTestCapture(figma)
    lockPath = path.join(figma, 'test-capture.lock.json')
    modulePaths = [
      modulePath,
      ledgerPath,
      resolverPath,
      ...runtimeFiles.map((file) => 'library/ui/figma/review/src/components/' + file)
    ]
    vi.mocked(componentModulePaths).mockReset().mockReturnValue(modulePaths)
    vi.mocked(isComponentModulePath)
      .mockReset()
      .mockImplementation(
        (
          await vi.importActual<typeof import('../component-generation.mjs')>(
            '../component-generation.mjs'
          )
        ).isComponentModulePath
      )
    await writeFile(path.join(repository, modulePath), 'export const fixtureGenerator = true\n')
    await mkdir(path.join(figma, 'ledger'), { recursive: true })
    await writeFile(path.join(repository, ledgerPath), '{"schema":1,"retired":[]}')
    await writeFile(path.join(repository, resolverPath), '{"compilerOptions":{}}')
    await mkdir(path.join(figma, 'review/src/components'), { recursive: true })
    for (const file of runtimeFiles)
      await writeFile(
        path.join(figma, 'review/src/components', file),
        file.endsWith('.css') ? '.root { display: block }' : "export * from '../generated/icons'"
      )
    registryPath = path.join(root, 'components/promotion.json')
    await mkdir(path.dirname(registryPath), { recursive: true })
    originalRegistry = Buffer.from(' { "components": [], "internal": ["DemoNav"] } ')
    await writeFile(registryPath, originalRegistry)
    await writeFile(path.join(root, 'index.ts'), 'export const foundationsOnly = true\n')
    await writeFile(path.join(root, 'package.json'), '{"name":"@fixture/ui","version":"0.0.0"}')
    await writeFile(path.join(root, 'pnpm-lock.yaml'), 'lockfileVersion: 9')
    await writeFile(path.join(root, 'tsconfig.json'), '{"compilerOptions":{}}')
    await symlink(path.resolve('node_modules'), path.join(root, 'node_modules'), 'dir')
    await writeFile(path.join(repository, '.gitignore'), 'node_modules/\n')
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    lock.sources[0].files = {}
    for (const file of [...modulePaths, 'library/ui/figma/figma.config.json'])
      lock.sources[0].files[file] = { sha256: hash(await readFile(path.join(repository, file))) }
    await writeFile(lockPath, JSON.stringify(lock))
    await initializeFixtureGit(repository)
    app = await createApp(figma)
    options = { target: 'components' }
  })

  afterEach(async () => {
    vi.restoreAllMocks()
    await rm(repository, { recursive: true, force: true })
  })

  it('should install internal output and its independent generation manifest with a second no-op', async () => {
    const first = await runInstall(app, options)
    const second = await runInstall(app, options)
    expect(first.changed).toBe(true)
    expect(second.changed).toBe(false)
    expect(second.files).toEqual(first.files)
    expect(first.files.map((file) => file.path)).toContain('generation.json')
    expect(first.files.map((file) => file.path)).not.toContain('DemoNav/DemoNav.figma.ts')
    expect(await readFile(path.join(root, 'index.ts'), 'utf8')).toBe(
      'export const foundationsOnly = true\n'
    )
    expect(JSON.parse(await readFile(registryPath, 'utf8'))).toEqual({
      components: [],
      internal: ['DemoNav']
    })
    expect(vi.mocked(createComponentGeneration).mock.calls[0]![0].modules).toHaveLength(
      modulePaths.length
    )
  })

  it('should refuse a registry with limits on an internal name and leave the installed output byte-identical', async () => {
    await runInstall(app, options)
    await fixtureCommit(repository)
    const snapshot = async () =>
      Object.fromEntries(
        await Promise.all(
          (await readdir(path.join(root, 'components'), { recursive: true }))
            .filter((file) => file !== 'promotion.json')
            .sort()
            .map(async (file) => {
              const full = path.join(root, 'components', file)
              return [file, (await lstat(full)).isFile() ? hash(await readFile(full)) : 'dir']
            })
        )
      )
    const before = await snapshot()
    await writeFile(
      registryPath,
      JSON.stringify({
        components: [],
        internal: [{ name: 'DemoNav', limits: [{ id: 'a', text: 'x.' }] }]
      })
    )
    await fixtureCommit(repository)

    await expect(runInstall(app, options)).rejects.toThrow('names only, without limits')

    expect(await snapshot()).toEqual(before)
  })

  it('should retire the last component through the same canonical transaction and then no-op', async () => {
    await runInstall(app, options)
    await fixtureCommit(repository)
    await writeFile(registryPath, '{"components":[]}')
    await fixtureCommit(repository)
    const retired = await runInstall(app, options)
    expect(retired.changed).toBe(true)
    expect(retired.files.map((file) => file.path)).toEqual(['generation.json', 'promotion.json'])
    expect(await readdir(path.join(root, 'components'))).toEqual([
      'generation.json',
      'promotion.json'
    ])
    expect((await runInstall(app, options)).changed).toBe(false)
    expect(await readFile(path.join(root, 'index.ts'), 'utf8')).toBe(
      'export const foundationsOnly = true\n'
    )
  })

  it('should reproduce empty retirement in a fresh immutable private stage and no-op', async () => {
    await writeFile(registryPath, '{"components":[],"internal":[]}')
    options.outDir = stage
    const first = await runInstall(app, options)
    expect(first.changed).toBe(true)
    expect(first.files.map((file) => file.path)).toEqual(['generation.json', 'promotion.json'])
    expect(await readdir(first.outDir)).toEqual(['generation.json', 'promotion.json'])
    expect((await runInstall(app, options)).changed).toBe(false)
    expect(await readFile(registryPath, 'utf8')).toBe('{"components":[],"internal":[]}')
  })

  it('should still refuse an unverified cache for an empty requested set and preserve old output', async () => {
    await runInstall(app, options)
    await fixtureCommit(repository)
    await writeFile(registryPath, '{"components":[]}')
    await fixtureCommit(repository)
    await rm(path.join(figma, 'figma-snapshot/.fds-source.json'))
    app = await createApp(figma, undefined, { unverifiedSnapshot: true })
    await expect(runInstall(app, options)).rejects.toThrow('not materialized')
    expect(await readdir(path.join(root, 'components'))).toEqual([
      'DemoNav',
      'generation.json',
      'promotion.json',
      'runtime'
    ])
    expect(await readFile(registryPath, 'utf8')).toBe('{"components":[]}')
  })

  it('should restore old component and runtime output when empty publication fails', async () => {
    await runInstall(app, options)
    await fixtureCommit(repository)
    await writeFile(registryPath, '{"components":[]}')
    await fixtureCommit(repository)
    vi.mocked(rename)
      .mockImplementationOnce(actualRename)
      .mockRejectedValueOnce(new Error('empty publication failed'))
    await expect(runInstall(app, options)).rejects.toThrow('empty publication failed')
    expect(await readdir(path.join(root, 'components'))).toEqual([
      'DemoNav',
      'generation.json',
      'promotion.json',
      'runtime'
    ])
    expect(await readFile(registryPath, 'utf8')).toBe('{"components":[]}')
    expect((await runInstall(app, options)).files.map((file) => file.path)).toEqual([
      'generation.json',
      'promotion.json'
    ])
    expect(await readdir(path.join(root, 'components'))).toEqual([
      'generation.json',
      'promotion.json'
    ])
  })

  it('should clone producer buffers before passing through provenance and publication', async () => {
    let producerBytes: Buffer
    let expectedBytes: Buffer
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      producerBytes = plan.files.get('DemoNav/DemoNav.tsx')!
      expectedBytes = Buffer.from(producerBytes)
      return plan
    })
    vi.mocked(createComponentGeneration).mockImplementationOnce(() => {
      producerBytes.fill(0)
      return Buffer.from('{}')
    })
    await runInstall(app, options)
    expect(await readFile(path.join(root, 'components/DemoNav/DemoNav.tsx'))).toEqual(
      expectedBytes!
    )
  })

  it('should reproduce a private stage without replacing the maintained registry', async () => {
    options.outDir = stage
    expect((await runInstall(app, options)).changed).toBe(true)
    expect((await runInstall(app, options)).changed).toBe(false)
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject config-file replacement during the first verification before planning', async () => {
    vi.mocked(verifyFdsCache).mockImplementationOnce(async (directory, lock) => {
      await actualCache(directory, lock)
      await writeFile(configPath, (await readFile(configPath, 'utf8')) + ' ')
    })
    await expect(runInstall(app, options)).rejects.toThrow('configuration changed')
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject normalized app-config drift during initial verification', async () => {
    vi.mocked(verifyFdsCache).mockImplementationOnce(async (directory, lock) => {
      await actualCache(directory, lock)
      app.config.imageFills = 'assets'
    })
    await expect(runInstall(app, options)).rejects.toThrow('configuration changed')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject a config file already different from the loaded app', async () => {
    await writeFile(configPath, JSON.stringify({ ...app.config, imageFills: 'assets' }))
    await expect(runInstall(app, options)).rejects.toThrow('loaded configuration differs')
    expect(verifyFdsCache).not.toHaveBeenCalled()
  })

  it('should reject a lock replaced during initial verification', async () => {
    vi.mocked(verifyFdsCache).mockImplementationOnce(async (directory, lock) => {
      await actualCache(directory, lock)
      await writeFile(lockPath, (await readFile(lockPath, 'utf8')) + ' ')
    })
    await expect(runInstall(app, options)).rejects.toThrow('source lock changed')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should recheck config after source readiness even when the source has its own verifier', async () => {
    const source = app.createSource('snapshot')
    const ready = source.ready!.bind(source)
    source.ready = async () => {
      await ready()
      app.config.imageFills = 'assets'
    }
    app.createSource = () => source
    await expect(runInstall(app, options)).rejects.toThrow('configuration changed')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject retirement-ledger drift before planning', async () => {
    await writeFile(path.join(repository, ledgerPath), '{"schema":1,"retired":[]} ')
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject retirement-ledger drift during planning without publication', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(path.join(repository, ledgerPath), '{"schema":1,"retired":[]} ')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
    expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
  })

  it('should reject a missing effective retirement ledger', async () => {
    await rm(path.join(repository, ledgerPath))
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject an untracked effective retirement ledger', async () => {
    await fixtureGit(repository, 'rm', '--cached', ledgerPath)
    await expect(runInstall(app, options)).rejects.toThrow('module inventory differs')
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject an omitted transitive source pin from the independent index inventory', async () => {
    await writeFile(path.join(figma, 'src/specs.ts'), 'export const transitive = true')
    await fixtureCommit(repository)
    await expect(runInstall(app, options)).rejects.toThrow(
      'implementation is absent from the source lock'
    )
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should recheck the independently inventoried source set after planning', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(path.join(figma, 'src/specs.ts'), 'export const transitive = true')
      await fixtureGit(repository, 'add', 'library/ui/figma/src/specs.ts')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow(
      'implementation is absent from the source lock'
    )
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it.each(['mts', 'MTS'])(
    'should reject the Audit-style untracked .%s helper override without publication',
    async (extension) => {
      await writeFile(
        path.join(figma, 'component-generation.' + extension),
        "export * from './component-generation.original.mjs';\nexport function createComponentGeneration() { throw new Error('AUDIT_UNTRACKED_SHADOW') }\n"
      )
      vi.mocked(isComponentModulePath).mockReturnValue(false)
      options.outDir = stage
      await expect(runInstall(app, options)).rejects.toThrow(
        'Untracked or ignored component implementation override'
      )
      expect(planComponents).not.toHaveBeenCalled()
      expect(createComponentGeneration).not.toHaveBeenCalled()
      await expect(readdir(path.join(root, '.agent-work'))).rejects.toThrow()
      expect(await readFile(registryPath)).toEqual(originalRegistry)
    }
  )

  it('should reject an ignored top-level shadow before planning', async () => {
    await writeFile(path.join(repository, '.gitignore'), '*.mts\n')
    await writeFile(path.join(figma, 'component-generation.mts'), 'export const ignored = true')
    await expect(runInstall(app, options)).rejects.toThrow(
      'Untracked or ignored component implementation override'
    )
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject ignored recursive source overrides without following Git exclusions', async () => {
    await writeFile(path.join(repository, '.gitignore'), '*.tsx\n')
    await mkdir(path.join(figma, 'src/nested'), { recursive: true })
    await writeFile(path.join(figma, 'src/nested/helper.tsx'), 'export const ignored = true')
    await expect(runInstall(app, options)).rejects.toThrow(
      'Untracked or ignored component implementation override'
    )
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject a shadow introduced during planning before any publication', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(path.join(figma, 'component-generation.mts'), 'export const late = true')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow(
      'Untracked or ignored component implementation override'
    )
    expect(await readFile(registryPath)).toEqual(originalRegistry)
    expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
  })

  it('should reject a tracked top-level implementation omitted from the locked read-set', async () => {
    await writeFile(path.join(figma, 'extra-helper.mts'), 'export const tracked = true')
    await fixtureCommit(repository)
    await expect(runInstall(app, options)).rejects.toThrow(
      'implementation is absent from the source lock'
    )
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it.each(['', 'src/', 'src/generators/'])(
    'should reject a package self-reference in %s with a non-src helper without publication',
    async (scope) => {
      await mkdir(path.join(figma, scope), { recursive: true })
      await mkdir(path.join(figma, 'audit-dependency'), { recursive: true })
      await writeFile(path.join(figma, 'audit-dependency/postcss.mjs'), 'export default {}')
      await writeFile(
        path.join(figma, scope, 'package.json'),
        JSON.stringify({
          name: 'postcss',
          exports: { '.': './audit-dependency/postcss.mjs' }
        })
      )
      options.outDir = stage
      await expect(runInstall(app, options)).rejects.toThrow('Unexpected component resolver scope')
      expect(planComponents).not.toHaveBeenCalled()
      expect(await readFile(registryPath)).toEqual(originalRegistry)
      await expect(readdir(path.join(root, '.agent-work'))).rejects.toThrow()
    }
  )

  it.each([
    'src/node_modules',
    'src/generators/node_modules',
    'node_modules',
    'src/jsconfig.json',
    'src/tsconfig.paths.json'
  ])('should reject an ignored resolver override %s without publication', async (override) => {
    await writeFile(path.join(repository, '.gitignore'), 'node_modules/\n*.json\n')
    await mkdir(path.dirname(path.join(figma, override)), { recursive: true })
    if (override.endsWith('node_modules'))
      await symlink(path.resolve('node_modules'), path.join(figma, override), 'dir')
    else await writeFile(path.join(figma, override), '{}')
    await expect(runInstall(app, options)).rejects.toThrow('Unexpected component resolver scope')
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject a UI package self-reference outside its installed dependency tree', async () => {
    await writeFile(path.join(root, 'audit-postcss.cjs'), 'module.exports = {}')
    await writeFile(
      path.join(root, 'package.json'),
      '{"name":"postcss","exports":"./audit-postcss.cjs"}'
    )
    await expect(runInstall(app, options)).rejects.toThrow('outside the sanctioned UI installation')
    expect(planComponents).not.toHaveBeenCalled()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
    expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
  })

  it('should recheck package resolution scopes after planning and preserve the old tree', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(
        path.join(figma, 'package.json'),
        '{"name":"postcss","exports":"./audit-dependency/postcss.mjs"}'
      )
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('Unexpected component resolver scope')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
    expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
  })

  it.each(['figma/tsconfig.json', 'tsconfig.json'])(
    'should reject dirty resolver configuration %s before planning',
    async (config) => {
      await writeFile(
        path.join(root, config),
        '{"compilerOptions":{"paths":{"postcss":["./synthetic-alias.ts"]}}}'
      )
      await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
      expect(planComponents).not.toHaveBeenCalled()
      expect(await readFile(registryPath)).toEqual(originalRegistry)
      expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
    }
  )

  it.each(['figma/tsconfig.json', 'tsconfig.json'])(
    'should reject resolver configuration %s changed during planning without publication',
    async (config) => {
      vi.mocked(planComponents).mockImplementationOnce(async (context) => {
        const plan = await actualPlan(context)
        await writeFile(
          path.join(root, config),
          '{"compilerOptions":{"paths":{"postcss":["./synthetic-alias.ts"]}}}'
        )
        return plan
      })
      await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
      expect(await readFile(registryPath)).toEqual(originalRegistry)
      expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
    }
  )

  it('should reject dirty committed module source before planning', async () => {
    await writeFile(path.join(repository, modulePath), 'changed')
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject dirty dependency bytes before planning', async () => {
    await writeFile(path.join(root, 'pnpm-lock.yaml'), 'changed dependency versions')
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should recheck committed dependencies after planning', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(path.join(root, 'package.json'), '{}')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should install with a locked tooling module that is outside the read-set', async () => {
    const tool = 'library/ui/figma/src/unrelated-tool.ts'
    await writeFile(path.join(repository, tool), 'export const tool = true\n')
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    lock.sources[0].files[tool] = { sha256: hash('export const tool = true\n') }
    await writeFile(lockPath, JSON.stringify(lock))
    await fixtureCommit(repository)
    const result = await runInstall(app, options)
    expect(result.changed).toBe(true)
    expect(
      vi
        .mocked(createComponentGeneration)
        .mock.calls[0]![0].modules.map((module: { path: string }) => module.path)
    ).not.toContain(tool)
  })

  it('should reject a missing module pin before planning', async () => {
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    delete lock.sources[0].files[modulePath]
    await writeFile(lockPath, JSON.stringify(lock))
    await expect(runInstall(app, options)).rejects.toThrow(
      'Component implementation is absent from the source lock'
    )
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject a pinned module whose committed bytes do not match', async () => {
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    lock.sources[0].files[modulePath].sha256 = '0'.repeat(64)
    await writeFile(lockPath, JSON.stringify(lock))
    await expect(runInstall(app, options)).rejects.toThrow('Unverified component input')
    expect(planComponents).not.toHaveBeenCalled()
  })

  it('should reject registry changes made by a producer with no supplied recheck hook', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(registryPath, '{}')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('component registry changed')
    expect(await readFile(registryPath, 'utf8')).toBe('{}')
    await expect(readdir(path.join(root, 'components/DemoNav'))).rejects.toThrow()
  })

  it('should reject runtime drift after a pure producer returns', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(
        path.join(figma, 'review/src/components', runtimeFiles[0]!),
        'runtime changed'
      )
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('Commit component source changes')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject cache drift after planning', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(path.join(figma, 'figma-snapshot/variables.json'), '[]')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow()
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should recheck normalized app-config identity after planning', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      app.config.imageFills = 'assets'
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('configuration changed')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject raw config drift after planning', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(configPath, (await readFile(configPath, 'utf8')) + ' ')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('configuration changed')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it('should reject lock drift after planning', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      await writeFile(lockPath, (await readFile(lockPath, 'utf8')) + ' ')
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('source lock changed')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it.each(['../escape.ts', '/absolute.ts', 'C:/drive.ts', 'x\\escape.ts', 'a/../b.ts', 'NUL.ts'])(
    'should reject unsafe producer output %s without writing',
    async (output) => {
      vi.mocked(planComponents).mockImplementationOnce(async (context) => {
        const plan = await actualPlan(context)
        plan.files.set(output, Buffer.from('bad'))
        return plan
      })
      await expect(runInstall(app, options)).rejects.toThrow('Unsafe install input or output path')
      expect(await readFile(registryPath)).toEqual(originalRegistry)
    }
  )

  it('should reject pointer payloads emitted by a producer before writing', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      plan.files.set(
        'pointer.ts',
        Buffer.from(
          'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'a'.repeat(64) + '\nsize 1\n'
        )
      )
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('output is an unmaterialized pointer')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
  })

  it.each(['', '{}'])(
    'should propagate invalid registry %j without creating component output',
    async (registry) => {
      await writeFile(registryPath, registry)
      await expect(runInstall(app, options)).rejects.toThrow()
      expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
      expect(await readFile(registryPath, 'utf8')).toBe(registry)
    }
  )

  it('should reject a producer attempting to supply its own provenance', async () => {
    vi.mocked(planComponents).mockImplementationOnce(async (context) => {
      const plan = await actualPlan(context)
      plan.files.set('generation.json', Buffer.from('{}'))
      return plan
    })
    await expect(runInstall(app, options)).rejects.toThrow('Invalid component install output')
  })

  it('should refuse a dirty public output and preserve it', async () => {
    await writeFile(path.join(root, 'components/user.ts'), 'keep')
    await expect(runInstall(app, options)).rejects.toThrow('dirty')
    expect(await readFile(path.join(root, 'components/user.ts'), 'utf8')).toBe('keep')
  })

  it('should refuse a symlinked registry before following it', async () => {
    await rm(registryPath)
    await symlink(configPath, registryPath)
    await expect(runInstall(app, options)).rejects.toThrow('symlink')
  })

  it('should refuse an unmaterialized registry pointer', async () => {
    await writeFile(
      registryPath,
      'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'a'.repeat(64) + '\nsize 1\n'
    )
    await expect(runInstall(app, options)).rejects.toThrow('unmaterialized pointer')
  })

  it('should refuse a symlinked private output', async () => {
    await mkdir(path.join(root, '.agent-work/figma-install/components-proof'), { recursive: true })
    await symlink(
      path.join(root, 'components'),
      path.join(root, '.agent-work/figma-install/components-proof/components')
    )
    options.outDir = stage
    await expect(runInstall(app, options)).rejects.toThrow('symlink')
  })

  it('should preserve another installer mutex on busy refusal', async () => {
    await writeFile(path.join(root, 'components.install-lock'), 'other installer')
    await expect(runInstall(app, options)).rejects.toThrow('busy')
    expect(await readFile(path.join(root, 'components.install-lock'), 'utf8')).toBe(
      'other installer'
    )
  })

  it('should restore the previous component directory on publication I/O failure', async () => {
    vi.mocked(rename)
      .mockImplementationOnce(actualRename)
      .mockRejectedValueOnce(new Error('component publication failed'))
    await expect(runInstall(app, options)).rejects.toThrow('component publication failed')
    expect(await readFile(registryPath)).toEqual(originalRegistry)
    expect(await readdir(path.join(root, 'components'))).toEqual(['promotion.json'])
    expect((await runInstall(app, options)).changed).toBe(true)
  })
})
