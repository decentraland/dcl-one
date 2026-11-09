import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, readdir, rename, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, type App } from './app.js'
import { runInstall } from './install.js'
import { copySnapshot, pinTestCapture, testConfig } from '../test/snapshot.js'
import { initializeFixtureGit } from './guard/git-fixture.js'

vi.mock('node:fs/promises', async (original) => {
  const actual = await original<typeof import('node:fs/promises')>()
  return { ...actual, rename: vi.fn(actual.rename) }
})

describe('when foundations are installed from a verified synthetic cache', () => {
  let root: string
  let figma: string
  let app: App
  let lockPath: string
  let baseline: string[]
  const entry = '@layer dcl.reset, dcl.foundations, dcl.components;\n'

  beforeEach(async () => {
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
    lock.publicFoundations = {
      files: {
        'foundations.css': {
          sha256: createHash('sha256').update(entry).digest('hex'),
          origin: { kind: 'source' }
        }
      }
    }
    await writeFile(lockPath, JSON.stringify(lock))
    await mkdir(path.join(root, 'foundations'))
    await writeFile(path.join(root, 'foundations/foundations.css'), entry)
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
    await expect(runInstall(app, { target: 'foundations' })).rejects.toThrow('rollback failed')
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
