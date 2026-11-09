import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, readdir, rename, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, type App } from './app.js'
import { verifyFdsCache } from './sources/fds-source.js'
import { runInstall } from './install.js'
import { copySnapshot, pinTestCapture, testConfig } from '../test/snapshot.js'
import { initializeFixtureGit } from './guard/git-fixture.js'

vi.mock('node:fs/promises', async (original) => {
  const actual = await original<typeof import('node:fs/promises')>()
  return { ...actual, rename: vi.fn(actual.rename) }
})

vi.mock('./sources/fds-source.js', async (original) => {
  const actual = await original<typeof import('./sources/fds-source.js')>()
  return { ...actual, verifyFdsCache: vi.fn(actual.verifyFdsCache) }
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
    await initializeFixtureGit(root)
    app = await createApp(figma)
    baseline = await readdir(path.join(root, 'foundations'))
  })

  afterEach(async () => {
    vi.restoreAllMocks()
    await rm(root, { recursive: true, force: true })
  })

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

  it.each(['', 'components', '../foundations'])(
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
