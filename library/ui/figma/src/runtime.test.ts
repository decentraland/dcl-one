import { mkdir, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { resolveComponent } from './contract/resolve.js'
import { makeLibrary } from './contract/test-fixtures.js'
import type { ComponentSpec } from './ir/components.js'
import { LfsPointerError } from './lfs.js'
import { FdsCacheError, MARKER } from './sources/fds-source.js'
import { createRuntime, supersessionCandidates } from './runtime.js'
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
