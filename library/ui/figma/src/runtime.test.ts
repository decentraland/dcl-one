import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { resolveComponent } from './contract/resolve.js'
import { makeLibrary } from './contract/test-fixtures.js'
import type { ComponentSpec } from './ir/components.js'
import { LfsPointerError } from './lfs.js'
import { FdsCacheError, MARKER } from './sources/fds-source.js'
import { LIBRARY_MAPPING_ROOT, createRuntime, supersessionCandidates } from './runtime.js'
import { copySnapshot, pinTestCapture, testConfig } from '../test/snapshot.js'

// Lets a test make loadSpecsFromSource fail with an arbitrary error; unset, the real loader runs.
const specsFailure = vi.hoisted(() => ({ error: undefined as Error | undefined }))
vi.mock('./specs.js', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./specs.js')>()
  return {
    ...actual,
    loadSpecsFromSource: (...args: Parameters<typeof actual.loadSpecsFromSource>) =>
      specsFailure.error ? Promise.reject(specsFailure.error) : actual.loadSpecsFromSource(...args)
  }
})

const spec = (name: string, extra: Partial<ComponentSpec> = {}) =>
  ({ name, page: 'Buttons', needsManual: [], ...extra }) as ComponentSpec

describe('supersessionCandidates', () => {
  describe('when a ready page generates an eligible component that promotion.include does not name', () => {
    it('should leave the hand-written mapping of that name to be validated', () => {
      expect(supersessionCandidates([spec('Button')], ['NavBar'], ['NavBar'])).toEqual(new Map())
    })
  })

  describe('when promotion.include is absent', () => {
    it('should supersede nothing', () => {
      expect(supersessionCandidates([spec('Button')], undefined, [])).toEqual(new Map())
    })
  })

  describe('when promotion.include names an eligible component not promoted yet', () => {
    it('should report it as superseding with no blockers', () => {
      expect(supersessionCandidates([spec('Button')], ['Button'], [])).toEqual(
        new Map([['Button', []]])
      )
    })
  })

  describe('when promotion.include names a component promotion still excludes', () => {
    it('should report the reasons it stays pending', () => {
      expect(
        supersessionCandidates(
          [spec('Button', { needsManual: ['unresolved-instance:Icon'] })],
          ['Button'],
          []
        )
      ).toEqual(new Map([['Button', ['unresolved-instance:Icon']]]))
    })
  })

  describe('when the named component was already promoted', () => {
    it('should not report it, since its mapping is the synced one', () => {
      expect(supersessionCandidates([spec('NavBar')], ['NavBar'], ['NavBar'])).toEqual(new Map())
    })
  })
})

// Ported from figma-design-system e730fbc (runtime part), keyless: the configuration carries no file key.
describe('when distinguishing configured capture failures from resolve-only mode', () => {
  let library: Awaited<ReturnType<typeof makeLibrary>>
  beforeEach(async () => {
    library = await makeLibrary()
  })
  afterEach(async () => {
    specsFailure.error = undefined
    await rm(library.root, { recursive: true, force: true })
  })

  it('should propagate a malformed configured snapshot instead of returning a mapping-only identity', async () => {
    await writeFile(
      path.join(library.root, 'figma.config.json'),
      JSON.stringify({
        outDir: 'generated',
        pages: { Controls: { kind: 'components' } },
        contract: { mappingRoot: 'src' }
      })
    )
    await mkdir(path.join(library.root, 'figma-snapshot/components'), { recursive: true })
    await writeFile(
      path.join(library.root, 'figma-snapshot/components/Controls.json'),
      '{malformed capture'
    )
    const runtime = await createRuntime({ cwd: library.root, env: {} })
    const failure = runtime.resolveContext()
    await expect(failure).rejects.toThrow()
    await expect(failure).rejects.not.toBeInstanceOf(LfsPointerError)
  })

  it('should fail closed when the configured fetch-source cache is missing, naming the command that fills it', async () => {
    await writeFile(
      path.join(library.root, 'figma.config.json'),
      JSON.stringify({
        outDir: 'generated',
        snapshotLock: 'upstream.lock.json',
        pages: { Controls: { kind: 'components' } },
        contract: { mappingRoot: 'src' }
      })
    )
    await writeFile(
      path.join(library.root, 'upstream.lock.json'),
      JSON.stringify({
        sources: [
          {
            repository: 'https://github.com/decentraland/figma-design-system',
            revision: 'a'.repeat(40),
            snapshot: { revision: 'a'.repeat(40), digests: { 'pages.json': 'b'.repeat(64) } }
          }
        ]
      })
    )
    const runtime = await createRuntime({ cwd: library.root, env: {} })
    const failure = runtime.resolveContext()
    await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
    await expect(failure).rejects.toThrow('pnpm figma fetch-source')
  })

  it('should fail closed when the configured lock does not pin the capture', async () => {
    await writeFile(
      path.join(library.root, 'figma.config.json'),
      JSON.stringify({
        outDir: 'generated',
        snapshotLock: 'upstream.lock.json',
        pages: { Controls: { kind: 'components' } },
        contract: { mappingRoot: 'src' }
      })
    )
    await writeFile(path.join(library.root, 'upstream.lock.json'), JSON.stringify({ sources: [] }))
    const runtime = await createRuntime({ cwd: library.root, env: {} })
    await expect(runtime.resolveContext()).rejects.toBeInstanceOf(FdsCacheError)
  })

  it('should preserve mapping-only resolution when no Figma configuration exists', async () => {
    const runtime = await createRuntime({
      cwd: library.root,
      env: {},
      mappingRoot: library.mappingRoot
    })
    const context = await runtime.resolveContext()
    expect(context.figmaSets).toEqual([])
    expect(context.loadSyncedComponents).toBeUndefined()
    expect(await resolveComponent(context, { nodeId: '1:1' })).toMatchObject({
      ok: true,
      warnings: [expect.objectContaining({ code: 'figma-identity-unverified' })]
    })
  })

  it('should reject synced components from a pinned cache whose marker is null instead of resolving an empty map', async () => {
    const { pages } = await testConfig()
    await copySnapshot(path.join(library.root, 'figma-snapshot'))
    await writeFile(
      path.join(library.root, 'figma.config.json'),
      JSON.stringify({
        outDir: 'generated',
        snapshotDir: 'figma-snapshot',
        pages,
        contract: { mappingRoot: 'src' }
      })
    )
    await pinTestCapture(library.root)
    const runtime = await createRuntime({ cwd: library.root, env: {} })
    const context = await runtime.resolveContext()

    await writeFile(path.join(library.root, 'figma-snapshot', MARKER), 'null')

    await expect(context.loadSyncedComponents?.()).rejects.toBeInstanceOf(FdsCacheError)
    await expect(runtime.resolveContext()).rejects.toBeInstanceOf(FdsCacheError)
  })
  describe('when loading the synced components of a configured, pinned capture', () => {
    const pinnedRuntime = async () => {
      const { pages } = await testConfig()
      await copySnapshot(path.join(library.root, 'figma-snapshot'))
      await writeFile(
        path.join(library.root, 'figma.config.json'),
        JSON.stringify({
          outDir: 'generated',
          snapshotDir: 'figma-snapshot',
          pages,
          contract: { mappingRoot: 'src' }
        })
      )
      await pinTestCapture(library.root)
      return createRuntime({ cwd: library.root, env: {} })
    }

    it('should resolve a map when the capture loads', async () => {
      const context = await (await pinnedRuntime()).resolveContext()
      await expect(context.loadSyncedComponents?.()).resolves.toBeInstanceOf(Map)
    })

    describe('and the library registry marks a generated component internal', () => {
      let registry: string
      let fixtureRoot: string
      beforeEach(async () => {
        fixtureRoot = library.root
        library.root = path.join(fixtureRoot, 'figma')
        await mkdir(library.root)
        registry = path.join(fixtureRoot, 'components/promotion.json')
        await mkdir(path.dirname(registry), { recursive: true })
        await writeFile(registry, JSON.stringify({ components: [], internal: ['DemoNav'] }))
      })
      afterEach(async () => {
        await rm(fixtureRoot, { recursive: true, force: true })
      })
      it('should not treat an installed internal component as a pending public supersession', async () => {
        const runtime = await pinnedRuntime()
        runtime.app!.config.promotion = { ...runtime.app!.config.promotion, include: ['DemoNav'] }
        expect(await (await runtime.resolveContext()).loadSyncedComponents!()).toEqual(new Map())
      })
      it('should reject an invalid internal list instead of ignoring installed visibility', async () => {
        await writeFile(registry, JSON.stringify({ components: [], internal: 'DemoNav' }))
        const context = await (await pinnedRuntime()).resolveContext()
        await expect(context.loadSyncedComponents!()).rejects.toThrow(
          'library component registry name list'
        )
      })
    })
    it('should propagate an I/O error instead of resolving an empty map', async () => {
      const context = await (await pinnedRuntime()).resolveContext()
      specsFailure.error = Object.assign(new Error('EIO: i/o error, read'), { code: 'EIO' })
      await expect(context.loadSyncedComponents?.()).rejects.toThrow('EIO')
    })

    it('should propagate a schema error instead of resolving an empty map', async () => {
      const context = await (await pinnedRuntime()).resolveContext()
      specsFailure.error = new SyntaxError('Unexpected token } in JSON')
      await expect(context.loadSyncedComponents?.()).rejects.toBeInstanceOf(SyntaxError)
    })
  })
})

describe('when resolving with --mapping-only next to a configured capture', () => {
  let library: Awaited<ReturnType<typeof makeLibrary>>
  let cache: string
  // Path, size and sha256 of every entry under the cache, so a read-only run must leave it byte-identical.
  const digest = async (dir: string): Promise<string> => {
    const entries = await readdir(dir, { recursive: true, withFileTypes: true })
    const lines = await Promise.all(
      entries.map(async (entry) => {
        const full = path.join(entry.parentPath, entry.name)
        const rel = path.relative(dir, full)
        if (!entry.isFile()) return `${rel} ${entry.isDirectory() ? 'dir' : 'other'}`
        const bytes = await readFile(full)
        return `${rel} ${createHash('sha256').update(bytes).digest('hex')}`
      })
    )
    return lines.sort().join('\n')
  }
  beforeEach(async () => {
    library = await makeLibrary()
    const { pages } = await testConfig()
    cache = path.join(library.root, 'figma-snapshot')
    await copySnapshot(cache)
    await writeFile(
      path.join(library.root, 'figma.config.json'),
      JSON.stringify({
        outDir: 'generated',
        snapshotDir: 'figma-snapshot',
        pages,
        contract: { mappingRoot: 'src' }
      })
    )
    await pinTestCapture(library.root)
    // Corrupt the verified cache: the default mode must refuse it, --mapping-only must not touch it.
    await writeFile(path.join(cache, MARKER), '{"revision":"tampered"}')
  })
  afterEach(async () => {
    await rm(library.root, { recursive: true, force: true })
  })

  it('should fail closed without the flag and name both routes', async () => {
    const runtime = await createRuntime({ cwd: library.root, env: {} })
    const failure = runtime.resolveContext()
    await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
    await expect(failure).rejects.toThrow(/fetch-source.*--mapping-only/)
  })

  it('should resolve from the mappings without reading, repairing or deleting the cache', async () => {
    const before = await digest(cache)
    const runtime = await createRuntime({
      cwd: library.root,
      env: {},
      mappingOnly: true,
      mappingRoot: library.mappingRoot
    })
    const context = await runtime.resolveContext()
    const result = await resolveComponent(context, { nodeId: '1:1' })

    expect(runtime.app).toBeUndefined()
    expect(context.figmaSets).toEqual([])
    expect(context.loadSyncedComponents).toBeUndefined()
    expect(result).toMatchObject({
      ok: true,
      resolved: { provenance: { figma: { identity: 'mapping-only' } } },
      warnings: [expect.objectContaining({ code: 'figma-identity-unverified' })]
    })
    expect(await digest(cache)).toBe(before)
  })

  it('should default the mapping root to the library components directory', async () => {
    const runtime = await createRuntime({
      cwd: path.dirname(library.root),
      env: {},
      mappingOnly: true
    })
    expect((await runtime.resolveContext()).mappingRoot).toBe(LIBRARY_MAPPING_ROOT)
    expect(LIBRARY_MAPPING_ROOT).toBe(path.resolve(import.meta.dirname, '../../components'))
  })

  it('should prefer an explicit mapping root from the environment', async () => {
    const runtime = await createRuntime({
      cwd: library.root,
      env: { DCL_FIGMA_MAPPING_ROOT: 'src' },
      mappingOnly: true
    })
    expect((await runtime.resolveContext()).mappingRoot).toBe(library.mappingRoot)
  })

  describe.each([
    ['the working directory', {}, undefined],
    ['DCL_FIGMA_CONFIG', { DCL_FIGMA_CONFIG: 'figma.config.json' }, undefined],
    ['--config', {}, 'figma.config.json']
  ])('and the Figma config comes from %s', (_label, env, configPath) => {
    it('should keep its contract mapping root and library range without touching the cache', async () => {
      const before = await digest(cache)
      const config = JSON.parse(
        await readFile(path.join(library.root, 'figma.config.json'), 'utf8')
      )
      config.contract = { mappingRoot: 'src', libraryRange: '^9.0.0' }
      await writeFile(path.join(library.root, 'figma.config.json'), JSON.stringify(config))

      const runtime = await createRuntime({ cwd: library.root, env, configPath, mappingOnly: true })
      const context = await runtime.resolveContext()

      expect(context).toMatchObject({ mappingRoot: library.mappingRoot, libraryRange: '^9.0.0' })
      expect(await resolveComponent(context, { nodeId: '1:1' })).toMatchObject({
        ok: false,
        errors: [
          expect.objectContaining({
            details: expect.objectContaining({ reason: 'library-version' })
          })
        ]
      })
      expect(await digest(cache)).toBe(before)
    })
  })

  it.each([
    ['a non-string mapping root', { mappingRoot: 7 }],
    ['a non-string library range', { mappingRoot: 'src', libraryRange: false }],
    ['a contract that is not an object', 'src']
  ])('should refuse a config contract with %s', async (_label, contract) => {
    const config = JSON.parse(await readFile(path.join(library.root, 'figma.config.json'), 'utf8'))
    config.contract = contract
    await writeFile(path.join(library.root, 'figma.config.json'), JSON.stringify(config))

    const runtime = await createRuntime({ cwd: library.root, env: {}, mappingOnly: true })

    await expect(runtime.resolveContext()).rejects.toThrow(
      'contract must hold a string mappingRoot'
    )
  })

  it('should refuse an explicit config that does not exist', async () => {
    const runtime = await createRuntime({
      cwd: library.root,
      env: {},
      mappingOnly: true,
      configPath: 'missing.json'
    })

    await expect(runtime.resolveContext()).rejects.toThrow('Figma config not found')
  })
})
