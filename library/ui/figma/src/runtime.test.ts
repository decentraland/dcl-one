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
    vi.resetAllMocks()
    await rm(library.root, { recursive: true, force: true })
  })

  describe('and the configured snapshot is malformed', () => {
    let failure: Promise<unknown>
    beforeEach(async () => {
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
      failure = runtime.resolveContext()
      failure.catch(() => undefined)
    })

    // The capture is never parsed: a configured snapshot without a lock already fails closed.
    it('should propagate the configured-capture failure instead of returning a mapping-only identity', async () => {
      await expect(failure).rejects.toThrow('names no snapshotLock')
      await expect(failure).rejects.not.toBeInstanceOf(LfsPointerError)
    })
  })

  describe('and the configured fetch-source cache is missing', () => {
    let failure: Promise<unknown>
    beforeEach(async () => {
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
      failure = runtime.resolveContext()
      failure.catch(() => undefined)
    })

    it('should fail closed, naming the command that fills it', async () => {
      await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
      await expect(failure).rejects.toThrow('pnpm figma fetch-source')
    })
  })

  describe('and the configured lock does not pin the capture', () => {
    let failure: Promise<unknown>
    beforeEach(async () => {
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
        JSON.stringify({ sources: [] })
      )
      const runtime = await createRuntime({ cwd: library.root, env: {} })
      failure = runtime.resolveContext()
      failure.catch(() => undefined)
    })

    it('should fail closed with an unusable pin error', async () => {
      await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
      await expect(failure).rejects.toThrow('The Figma capture pin is unusable')
    })
  })

  describe('and no Figma configuration exists', () => {
    let context: Awaited<ReturnType<Awaited<ReturnType<typeof createRuntime>>['resolveContext']>>
    beforeEach(async () => {
      const runtime = await createRuntime({
        cwd: library.root,
        env: {},
        mappingRoot: library.mappingRoot
      })
      context = await runtime.resolveContext()
    })

    it('should expose no Figma sets', () => {
      expect(context.figmaSets).toEqual([])
    })

    it('should not offer synced components', () => {
      expect(context.loadSyncedComponents).toBeUndefined()
    })

    it('should preserve mapping-only resolution with an unverified identity warning', async () => {
      expect(await resolveComponent(context, { nodeId: '1:1' })).toMatchObject({
        ok: true,
        warnings: [expect.objectContaining({ code: 'figma-identity-unverified' })]
      })
    })
  })

  describe('and the marker of a pinned cache becomes null', () => {
    let runtime: Awaited<ReturnType<typeof createRuntime>>
    let context: Awaited<ReturnType<typeof runtime.resolveContext>>
    beforeEach(async () => {
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
      runtime = await createRuntime({ cwd: library.root, env: {} })
      context = await runtime.resolveContext()

      await writeFile(path.join(library.root, 'figma-snapshot', MARKER), 'null')
    })

    it('should reject synced components instead of resolving an empty map', async () => {
      const failure = context.loadSyncedComponents?.()
      await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
      await expect(failure).rejects.toThrow('The Figma capture marker is malformed')
    })

    it('should reject a new context with a malformed marker error', async () => {
      const failure = runtime.resolveContext()
      await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
      await expect(failure).rejects.toThrow('The Figma capture marker is malformed')
    })
  })

  describe('and the synced components of a configured, pinned capture are loaded', () => {
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

    describe('and the capture loads', () => {
      it('should resolve a map', async () => {
        const context = await (await pinnedRuntime()).resolveContext()
        await expect(context.loadSyncedComponents?.()).resolves.toBeInstanceOf(Map)
      })
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

      describe('and promotion.include names that internal component', () => {
        let runtime: Awaited<ReturnType<typeof createRuntime>>
        beforeEach(async () => {
          runtime = await pinnedRuntime()
          runtime.app!.config.promotion = { ...runtime.app!.config.promotion, include: ['DemoNav'] }
        })

        it('should not treat the installed internal component as a pending public supersession', async () => {
          expect(await (await runtime.resolveContext()).loadSyncedComponents!()).toEqual(new Map())
        })
      })

      describe('and the internal list is not a list', () => {
        beforeEach(async () => {
          await writeFile(registry, JSON.stringify({ components: [], internal: 'DemoNav' }))
        })

        it('should reject it instead of ignoring installed visibility', async () => {
          const context = await (await pinnedRuntime()).resolveContext()
          await expect(context.loadSyncedComponents!()).rejects.toThrow(
            'library component registry name list'
          )
        })
      })
    })

    describe('and reading the specs fails with an I/O error', () => {
      let context: Awaited<ReturnType<Awaited<ReturnType<typeof pinnedRuntime>>['resolveContext']>>
      beforeEach(async () => {
        context = await (await pinnedRuntime()).resolveContext()
        specsFailure.error = Object.assign(new Error('EIO: i/o error, read'), { code: 'EIO' })
      })

      it('should propagate the I/O error instead of resolving an empty map', async () => {
        await expect(context.loadSyncedComponents?.()).rejects.toThrow('EIO')
      })
    })

    describe('and reading the specs fails with a schema error', () => {
      let context: Awaited<ReturnType<Awaited<ReturnType<typeof pinnedRuntime>>['resolveContext']>>
      beforeEach(async () => {
        context = await (await pinnedRuntime()).resolveContext()
        specsFailure.error = new SyntaxError('Unexpected token } in JSON')
      })

      it('should propagate the schema error instead of resolving an empty map', async () => {
        const failure = context.loadSyncedComponents?.()
        await expect(failure).rejects.toBeInstanceOf(SyntaxError)
        await expect(failure).rejects.toThrow('Unexpected token } in JSON')
      })
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
  const editConfig = async (edit: (config: Record<string, unknown>) => void) => {
    const config = JSON.parse(await readFile(path.join(library.root, 'figma.config.json'), 'utf8'))
    edit(config)
    await writeFile(path.join(library.root, 'figma.config.json'), JSON.stringify(config))
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

  describe('and the flag is not passed', () => {
    it('should fail closed and name both routes', async () => {
      const runtime = await createRuntime({ cwd: library.root, env: {} })
      const failure = runtime.resolveContext()
      await expect(failure).rejects.toBeInstanceOf(FdsCacheError)
      await expect(failure).rejects.toThrow(/fetch-source.*--mapping-only/)
    })
  })

  describe('and a mapping root is passed', () => {
    let before: string
    let runtime: Awaited<ReturnType<typeof createRuntime>>
    let context: Awaited<ReturnType<typeof runtime.resolveContext>>
    let result: Awaited<ReturnType<typeof resolveComponent>>
    beforeEach(async () => {
      before = await digest(cache)
      runtime = await createRuntime({
        cwd: library.root,
        env: {},
        mappingOnly: true,
        mappingRoot: library.mappingRoot
      })
      context = await runtime.resolveContext()
      result = await resolveComponent(context, { nodeId: '1:1' })
    })

    it('should not load the Figma app, its sets or its synced components', () => {
      expect(runtime.app).toBeUndefined()
      expect(context.figmaSets).toEqual([])
      expect(context.loadSyncedComponents).toBeUndefined()
    })

    it('should resolve from the mappings with a mapping-only identity', () => {
      expect(result).toMatchObject({
        ok: true,
        resolved: { provenance: { figma: { identity: 'mapping-only' } } },
        warnings: [expect.objectContaining({ code: 'figma-identity-unverified' })]
      })
    })

    it('should not read, repair or delete the cache', async () => {
      expect(await digest(cache)).toBe(before)
    })
  })

  describe('and no mapping root is given', () => {
    it('should default the mapping root to the library components directory', async () => {
      const runtime = await createRuntime({
        cwd: path.dirname(library.root),
        env: {},
        mappingOnly: true
      })
      expect((await runtime.resolveContext()).mappingRoot).toBe(LIBRARY_MAPPING_ROOT)
      expect(LIBRARY_MAPPING_ROOT).toBe(path.resolve(import.meta.dirname, '../../components'))
    })
  })

  describe('and the environment names a mapping root', () => {
    it('should prefer the explicit mapping root from the environment', async () => {
      const runtime = await createRuntime({
        cwd: library.root,
        env: { DCL_FIGMA_MAPPING_ROOT: 'src' },
        mappingOnly: true
      })
      expect((await runtime.resolveContext()).mappingRoot).toBe(library.mappingRoot)
    })
  })

  describe.each([
    ['the working directory', {}, undefined],
    ['DCL_FIGMA_CONFIG', { DCL_FIGMA_CONFIG: 'figma.config.json' }, undefined],
    ['--config', {}, 'figma.config.json']
  ])('and the Figma config comes from %s', (_label, env, configPath) => {
    let before: string
    beforeEach(async () => {
      before = await digest(cache)
      await editConfig((config) => {
        config.contract = { mappingRoot: 'src', libraryRange: '^9.0.0' }
      })
    })

    it('should keep its contract mapping root and library range without touching the cache', async () => {
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

  describe.each([
    ['a non-string mapping root', { mappingRoot: 7 }],
    ['a non-string library range', { mappingRoot: 'src', libraryRange: false }],
    ['a contract that is not an object', 'src']
  ])('and the config contract has %s', (_label, contract) => {
    beforeEach(async () => {
      await editConfig((config) => {
        config.contract = contract
      })
    })

    it('should refuse the contract', async () => {
      const runtime = await createRuntime({ cwd: library.root, env: {}, mappingOnly: true })

      await expect(runtime.resolveContext()).rejects.toThrow(
        'contract must hold a string mappingRoot'
      )
    })
  })

  describe('and the config holds componentNames aliases', () => {
    let context: Awaited<ReturnType<Awaited<ReturnType<typeof createRuntime>>['resolveContext']>>
    beforeEach(async () => {
      await editConfig((config) => {
        config.componentNames = { 'Button-Contained-Text': 'Button' }
      })
      const runtime = await createRuntime({ cwd: library.root, env: {}, mappingOnly: true })
      context = await runtime.resolveContext()
    })

    it('should carry the aliases in the context', () => {
      expect(context.componentNames).toEqual({ 'Button-Contained-Text': 'Button' })
    })

    it('should resolve a name lookup through an alias', async () => {
      expect(
        await resolveComponent(context, {
          name: 'Button-Contained-Text',
          variantProperties: { Kind: 'Primary' }
        })
      ).toMatchObject({ ok: true })
    })
  })

  describe('and the config componentNames are not a name map', () => {
    beforeEach(async () => {
      await editConfig((config) => {
        config.componentNames = { Button: 7 }
      })
    })

    it('should refuse them', async () => {
      const runtime = await createRuntime({ cwd: library.root, env: {}, mappingOnly: true })

      await expect(runtime.resolveContext()).rejects.toThrow(
        'componentNames must map names to names'
      )
    })
  })

  describe('and an explicit config is missing', () => {
    it.each([
      ['--config', { configPath: 'missing.json', env: {} }],
      ['DCL_FIGMA_CONFIG', { env: { DCL_FIGMA_CONFIG: 'missing.json' } }]
    ])('should refuse the config from %s at startup', async (_label, options) => {
      await expect(
        createRuntime({ cwd: library.root, mappingOnly: true, ...options })
      ).rejects.toThrow('Figma config not found')
    })
  })

  describe('and the config file is not JSON', () => {
    beforeEach(async () => {
      await writeFile(path.join(library.root, 'figma.config.json'), '{not json')
    })

    it('should name the config file in the error', async () => {
      const runtime = await createRuntime({ cwd: library.root, env: {}, mappingOnly: true })

      await expect(runtime.resolveContext()).rejects.toThrow(
        path.join(library.root, 'figma.config.json') + ':'
      )
    })
  })
})
