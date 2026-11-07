import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { cp, mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import {
  BUTTONS,
  PLACES,
  GEN,
  SNAP,
  components,
  manifest,
  put,
  writeTree
} from './test-fixtures.js'
import { checkScope } from './check-scope.js'
import { LEDGER } from './consumers.js'
import { listFiles, toPosix } from './fs-safe.js'
import { emptyRetirements } from './retirements.js'
import { GUARDED_DIRECTORIES, GUARDED_FILES, buildOwnership, ownerOf } from './ownership.js'

const repoRoot = path.resolve(import.meta.dirname, '../../..')

describe('ownership of the committed tree', () => {
  it('should identify managed artifacts while leaving orphaned legacy evidence unowned', async () => {
    const ownership = await buildOwnership(repoRoot)
    const files: string[] = [...GUARDED_FILES]
    for (const directory of GUARDED_DIRECTORIES)
      for (const file of await listFiles(path.join(repoRoot, directory)))
        files.push(toPosix(path.relative(repoRoot, file)))

    const unowned = files.filter(
      (file) => path.basename(file) !== '.gitkeep' && ownerOf(ownership, file) === null
    )

    expect(unowned).toHaveLength(104)
    expect(
      unowned.every((file) =>
        /^figma\/review\/public\/dcl-ui-review\/(references|renders)\/[^/]+\.png$/.test(file)
      )
    ).toBe(true)
  })

  it('should pass an automated run that changed nothing', async () => {
    const result = await checkScope({
      baseDir: repoRoot,
      stagedDir: repoRoot,
      capturedPageIds: [],
      runKind: 'automated',
      stage: 'complete',
      retirements: emptyRetirements()
    })

    expect(result).toEqual({ ok: true, violations: [] })
  })
})

describe('when the tree holds an orphaned asset ledger', () => {
  it('should own it as bookkeeping, so the committed tree keeps no unowned managed file', async () => {
    expect(ownerOf(await buildOwnership(repoRoot), LEDGER)).toEqual({ aggregate: 'orphan-ledger' })
  })
})

describe('when display metadata disagrees with stable ownership', () => {
  let root: string
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'ownership-id-'))
    await writeTree(root)
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should take a snapshot file owner from its embedded page ID instead of its filename', async () => {
    await put(root, 'figma/figma.config.json', {
      pages: { [BUTTONS]: { id: BUTTONS, name: 'Buttons', kind: 'components' } }
    })
    await put(root, `${SNAP}/components/Buttons.json`, {
      page: { id: PLACES, name: 'Renamed' },
      components: []
    })
    expect(ownerOf(await buildOwnership(root), `${SNAP}/components/Buttons.json`)).toEqual({
      pageId: PLACES
    })
  })
  it('should not infer generated ownership from a matching display name when its component ID is absent', async () => {
    await put(root, `${SNAP}/components/Buttons.json`, {
      page: { id: BUTTONS, name: 'Buttons' },
      components: []
    })
    expect(ownerOf(await buildOwnership(root), `${GEN}/components/buttons/Button.tsx`)).toBeNull()
  })
  it('should accept configured pages sharing a name without an ownership conflict', async () => {
    await put(root, 'figma/figma.config.json', {
      pages: {
        [BUTTONS]: { id: BUTTONS, name: 'Shared', kind: 'components' },
        [PLACES]: { id: PLACES, name: 'Shared', kind: 'components' }
      }
    })
    expect((await buildOwnership(root)).conflicts).toEqual([])
  })
  it('should retain the native component owner when an external reference copies its ID', async () => {
    await put(root, `${SNAP}/components/External.json`, {
      page: { id: 'external', name: 'External' },
      components: [{ id: 'b1', name: 'External copy', external: true }]
    })
    const ownership = await buildOwnership(root)
    expect(ownership.conflicts).toEqual([])
    expect(ownerOf(ownership, `${GEN}/components/buttons/Button.tsx`)).toEqual({
      pageId: BUTTONS,
      componentId: 'b1'
    })
    expect(ownerOf(ownership, `${SNAP}/components/External.json`)).toEqual({ pageId: 'external' })
  })
  it.each([false, true])(
    'should freeze unowned evidence even with a new staged identity claim: %s',
    async (claimed) => {
      await put(root, `${SNAP}/components/Other.json`, {
        page: { id: '9:1', name: 'Other' },
        components: [{ id: '9:2', name: 'Orphan' }]
      })
      const file = 'figma/review/public/dcl-ui-review/renders/Orphan-0.png'
      await put(root, file, 'retained')
      const staged = await mkdtemp(path.join(os.tmpdir(), 'unowned-evidence-'))
      try {
        await cp(root, staged, { recursive: true })
        await put(staged, file, 'changed')
        if (claimed) {
          await put(
            staged,
            `${GEN}/components/manifest.ts`,
            manifest([
              ...components,
              { name: 'Orphan', page: 'Other', id: '9:2', file: 'other/Orphan', variants: [] }
            ])
          )
          await put(staged, 'figma/review/public/dcl-ui-review/fidelity.json', {
            cases: { 'Orphan-0': { sourceId: '9:2', image: '/dcl-ui-review/renders/Orphan-0.png' } }
          })
        }
        expect(ownerOf(await buildOwnership(root), file)).toBeNull()
        const result = await checkScope({
          baseDir: root,
          stagedDir: staged,
          capturedPageIds: [BUTTONS, PLACES, '9:1'],
          runKind: 'automated',
          stage: 'complete',
          retirements: emptyRetirements()
        })
        expect(result.ok).toBe(false)
        expect(result.violations).toEqual(
          expect.arrayContaining([expect.objectContaining({ path: file, owner: null })])
        )
      } finally {
        await rm(staged, { recursive: true, force: true })
      }
    }
  )
  it('should not claim foreign files merely sharing a generated module prefix', async () => {
    const ownership = await buildOwnership(root)
    expect(ownerOf(ownership, `${GEN}/components/buttons/Button.custom.tsx`)).toBeNull()
    expect(ownerOf(ownership, `${GEN}/components/buttons/Button.figma.ts`)).toEqual({
      pageId: BUTTONS,
      componentId: 'b1'
    })
  })
  it('should report conflicting native page claims for the same component ID', async () => {
    await put(root, `${SNAP}/components/Other.json`, {
      page: { id: PLACES, name: 'Other' },
      components: [{ id: 'b1', name: 'Button' }]
    })
    expect((await buildOwnership(root)).conflicts.join(' ')).toContain(
      'componentPageById "b1" claimed as'
    )
  })
  it('should recognize shared library runtime only when prior promotion recorded the artifact', async () => {
    await put(root, 'components/promotion.json', {
      schema: 1,
      components: ['Button', 'PlaceCard'],
      files: { 'figma/review/src/components/figma-module-runtime.tsx': 'reviewed' }
    })
    const ownership = await buildOwnership(root)
    expect(ownerOf(ownership, 'components/runtime/figma-module-runtime.tsx')).toEqual({
      aggregate: 'runtime'
    })
    expect(ownerOf(ownership, 'components/runtime/Foreign.tsx')).toBeNull()
  })
  it('should reject an invalid embedded snapshot ID instead of assigning a filename owner', async () => {
    await put(root, `${SNAP}/components/Buttons.json`, {
      page: { id: 123, name: 'Buttons' },
      components: []
    })
    const ownership = await buildOwnership(root)
    expect(ownerOf(ownership, `${SNAP}/components/Buttons.json`)).toBeNull()
    expect(ownership.conflicts.join(' ')).toContain('Invalid snapshot page identity')
  })
})
