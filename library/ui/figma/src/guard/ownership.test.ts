import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
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
import {
  GENERATED,
  GUARDED_DIRECTORIES,
  GUARDED_FILES,
  buildOwnership,
  ownerOf
} from './ownership.js'
import { createApp, runSync } from '../app.js'
import { copySnapshot, testConfigFile } from '../../test/snapshot.js'

const repoRoot = path.resolve(import.meta.dirname, '../../..')

const unownedFiles = async (root: string) => {
  const ownership = await buildOwnership(root)
  const files: string[] = [...GUARDED_FILES]
  for (const directory of GUARDED_DIRECTORIES)
    for (const file of await listFiles(path.join(root, directory)))
      files.push(toPosix(path.relative(root, file)))
  return files.filter(
    (file) => path.basename(file) !== '.gitkeep' && ownerOf(ownership, file) === null
  )
}

// The committed Review is generated from the capture, which only maintainers materialize; a tree generated
// from the synthetic snapshot stands in for it.
describe('ownership of a generated tree', () => {
  let root: string
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'ownership-tree-'))
    const figma = path.join(root, 'figma')
    await mkdir(figma, { recursive: true })
    await copySnapshot(path.join(figma, 'figma-snapshot'))
    const config = JSON.parse(await readFile(testConfigFile, 'utf8'))
    await writeFile(
      path.join(figma, 'figma.config.json'),
      JSON.stringify({ ...config, snapshotDir: 'figma-snapshot', outDir: `../${GENERATED}` })
    )
    await runSync(
      await createApp(figma, path.join(figma, 'figma.config.json'), { unverifiedSnapshot: true }),
      {
        source: 'snapshot',
        developer: true
      }
    )
  }, 60_000)
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  it('should own every generated and captured artifact', async () => {
    const generated = await listFiles(path.join(root, GENERATED))

    expect(generated.length).toBeGreaterThan(0)
    expect(await unownedFiles(root)).toEqual([])
  })

  // The Review publishes no Figma evidence: a PNG anywhere under public/dcl-ui-review, including the old
  // references/, renders/ and icons/ folders, belongs to nothing.
  it.each([
    'figma/review/public/dcl-ui-review/planted.png',
    'figma/review/public/dcl-ui-review/references/live-1-1.png',
    'figma/review/public/dcl-ui-review/renders/Button-0.png',
    'figma/review/public/dcl-ui-review/icons/1-1.png'
  ])('should leave a PNG planted at %s unowned', async (planted) => {
    await mkdir(path.dirname(path.join(root, planted)), { recursive: true })
    await writeFile(path.join(root, planted), 'not a managed file')

    expect(await unownedFiles(root)).toEqual([planted])
  })
})

describe('ownership of the committed tree', () => {
  describe('when an automated run changed nothing', () => {
    it('should pass the scope check', async () => {
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

  describe('and a snapshot file is renamed away from its embedded page ID', () => {
    beforeEach(async () => {
      await put(root, 'figma/figma.config.json', {
        pages: { [BUTTONS]: { id: BUTTONS, name: 'Buttons', kind: 'components' } }
      })
      await put(root, `${SNAP}/components/Buttons.json`, {
        page: { id: PLACES, name: 'Renamed' },
        components: []
      })
    })

    it('should take the snapshot file owner from its embedded page ID instead of its filename', async () => {
      expect(ownerOf(await buildOwnership(root), `${SNAP}/components/Buttons.json`)).toEqual({
        pageId: PLACES
      })
    })
  })

  describe('and a generated file matches a display name whose component ID is absent', () => {
    beforeEach(async () => {
      await put(root, `${SNAP}/components/Buttons.json`, {
        page: { id: BUTTONS, name: 'Buttons' },
        components: []
      })
    })

    it('should not infer generated ownership from the display name', async () => {
      expect(ownerOf(await buildOwnership(root), `${GEN}/components/buttons/Button.tsx`)).toBeNull()
    })
  })

  describe('and configured pages share a name', () => {
    beforeEach(async () => {
      await put(root, 'figma/figma.config.json', {
        pages: {
          [BUTTONS]: { id: BUTTONS, name: 'Shared', kind: 'components' },
          [PLACES]: { id: PLACES, name: 'Shared', kind: 'components' }
        }
      })
    })

    it('should accept them without an ownership conflict', async () => {
      expect((await buildOwnership(root)).conflicts).toEqual([])
    })
  })

  describe('and an external reference copies a native component ID', () => {
    beforeEach(async () => {
      await put(root, `${SNAP}/components/External.json`, {
        page: { id: 'external', name: 'External' },
        components: [{ id: 'b1', name: 'External copy', external: true }]
      })
    })

    it('should retain the native component owner', async () => {
      const ownership = await buildOwnership(root)
      expect(ownership.conflicts).toEqual([])
      expect(ownerOf(ownership, `${GEN}/components/buttons/Button.tsx`)).toEqual({
        pageId: BUTTONS,
        componentId: 'b1'
      })
      expect(ownerOf(ownership, `${SNAP}/components/External.json`)).toEqual({ pageId: 'external' })
    })
  })

  describe('and a staged tree changes an unowned public PNG', () => {
    const file = 'figma/review/public/dcl-ui-review/renders/Orphan-0.png'
    let staged: string

    beforeEach(async () => {
      await put(root, `${SNAP}/components/Other.json`, {
        page: { id: '9:1', name: 'Other' },
        components: [{ id: '9:2', name: 'Orphan' }]
      })
      await put(root, file, 'retained')
      staged = await mkdtemp(path.join(os.tmpdir(), 'unowned-evidence-'))
      await cp(root, staged, { recursive: true })
      await put(staged, file, 'changed')
    })

    afterEach(async () => {
      await rm(staged, { recursive: true, force: true })
    })

    describe.each([false, true])(
      'and the staged tree claims a new identity for it: %s',
      (claimed) => {
        beforeEach(async () => {
          if (claimed) {
            await put(
              staged,
              `${GEN}/components/manifest.ts`,
              manifest([
                ...components,
                { name: 'Orphan', page: 'Other', id: '9:2', file: 'other/Orphan', variants: [] }
              ])
            )
          }
        })

        it('should leave the PNG unowned in the base tree', async () => {
          expect(ownerOf(await buildOwnership(root), file)).toBeNull()
        })

        it('should freeze the PNG with an unowned violation', async () => {
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
        })
      }
    )
  })

  describe('and a pull request plants a PNG under public/dcl-ui-review', () => {
    const file = 'figma/review/public/dcl-ui-review/renders/Button-0.png'
    let staged: string

    beforeEach(async () => {
      staged = await mkdtemp(path.join(os.tmpdir(), 'planted-png-'))
      await cp(root, staged, { recursive: true })
      await put(staged, file, 'planted')
    })

    afterEach(async () => {
      await rm(staged, { recursive: true, force: true })
    })

    it('should reject the planted PNG', async () => {
      const result = await checkScope({
        baseDir: root,
        stagedDir: staged,
        capturedPageIds: [BUTTONS, PLACES],
        runKind: 'automated',
        stage: 'complete',
        retirements: emptyRetirements()
      })

      expect(result.ok).toBe(false)
      expect(result.violations).toEqual(
        expect.arrayContaining([expect.objectContaining({ path: file, owner: null })])
      )
    })
  })

  describe('and a foreign file shares a generated module prefix', () => {
    it('should not claim the foreign file', async () => {
      expect(
        ownerOf(await buildOwnership(root), `${GEN}/components/buttons/Button.custom.tsx`)
      ).toBeNull()
    })

    it('should still own the generated module companion', async () => {
      expect(
        ownerOf(await buildOwnership(root), `${GEN}/components/buttons/Button.figma.ts`)
      ).toEqual({
        pageId: BUTTONS,
        componentId: 'b1'
      })
    })
  })

  describe('and native pages claim the same component ID', () => {
    beforeEach(async () => {
      await put(root, `${SNAP}/components/Other.json`, {
        page: { id: PLACES, name: 'Other' },
        components: [{ id: 'b1', name: 'Button' }]
      })
    })

    it('should report the conflicting page claims', async () => {
      expect((await buildOwnership(root)).conflicts.join(' ')).toContain(
        'componentPageById "b1" claimed as'
      )
    })
  })

  describe('and the embedded snapshot page ID is invalid', () => {
    beforeEach(async () => {
      await put(root, `${SNAP}/components/Buttons.json`, {
        page: { id: 123, name: 'Buttons' },
        components: []
      })
    })

    it('should not assign a filename owner', async () => {
      expect(ownerOf(await buildOwnership(root), `${SNAP}/components/Buttons.json`)).toBeNull()
    })

    it('should report the invalid snapshot page identity', async () => {
      expect((await buildOwnership(root)).conflicts.join(' ')).toContain(
        'Invalid snapshot page identity'
      )
    })
  })
})
