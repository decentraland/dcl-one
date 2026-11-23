import { cp, mkdtemp, readFile, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { runSync, type App } from '../app.js'
import { pendingRetirements } from './apply-retirements.js'
import { checkScope } from './check-scope.js'
import { emptyOwnership, type Ownership, type VariantParent } from './ownership.js'
import { reparentedTo } from './reparented-variants.js'
import { fixtureCommit, fixtureGit } from './git-fixture.js'
import { fixtureBytes, json, lifecycleFixture, put, GEN, SNAP } from './lifecycle-fixture.js'
import type { Retirements } from './retirements.js'

type SavedComponent = {
  kind: string
  id: string
  name: string
  base: { id: string; name: string; props: Record<string, string>; tree: unknown }
  others: { id: string; name: string; props: Record<string, string>; patches: unknown[] }[]
  properties: Record<string, unknown>
  truncated?: boolean
  external?: boolean
}

const PAGE = `${SNAP}/components/Page.json`

// A designer merges a retired component into a new set: its id lives on as a variant of the successor.
function successor(
  id: string,
  name: string,
  retired: SavedComponent,
  extra = '3:9'
): SavedComponent {
  return {
    ...retired,
    kind: 'set',
    id,
    name,
    properties: { State: { type: 'VARIANT', default: 'Old', options: ['Old', 'New'] } },
    base: { ...retired.base, name: 'State=Old', props: { State: 'Old' } },
    others: [{ id: extra, name: 'State=New', props: { State: 'New' }, patches: [] }]
  }
}

describe('when a retired component reappears as a variant of a live set', () => {
  let root: string
  let base: string
  let app: App
  let records: Retirements
  let shared: string
  let retired: SavedComponent

  beforeEach(async () => {
    ;({ root, app, records, shared } = await lifecycleFixture())
    retired = (await json(root, PAGE)).components[0]
    await put(root, `figma/ledger/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    await fixtureCommit(root, 'test: retire First')
    await fixtureGit(root, 'update-ref', 'refs/remotes/origin/main', 'HEAD')
    base = await mkdtemp(path.join(os.tmpdir(), 'variant-reparent-base-'))
    await cp(root, base, { recursive: true, verbatimSymlinks: true })
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
    await rm(base, { recursive: true, force: true })
  })

  // The cleanup leaves the pinned cache as it was; a later capture is where the retired component goes away.
  const recapture = async (components: (page: SavedComponent[]) => SavedComponent[]) => {
    const page = await json(root, PAGE)
    page.components = components(
      page.components.filter((component: SavedComponent) => component.id !== retired.id)
    )
    await put(root, PAGE, page)
  }
  const guard = () =>
    checkScope({
      baseDir: base,
      stagedDir: root,
      capturedPageIds: ['1:1'],
      runKind: 'automated',
      stage: 'complete',
      retirements: records
    })

  describe('and the successor has a different id on the same page', () => {
    beforeEach(async () => {
      await recapture((page) => [...page, successor('3:1', 'Merged', retired)])
      await runSync(app, { source: 'snapshot', capturedPageIds: ['1:1'] })
    })

    it('should capture the successor with nothing pending and no ownership violation', async () => {
      expect(await pendingRetirements(root, records)).toEqual([])
      expect(await guard()).toEqual({ ok: true, violations: [] })
      expect(await readFile(path.join(root, GEN, 'page/Merged.tsx'), 'utf8')).toContain('Merged')
    })

    it('should keep the successor through apply-retirements and repeat byte-identically', async () => {
      await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })

      const ids = (await json(root, PAGE)).components.map(
        (component: SavedComponent) => component.id
      )
      expect(ids).toEqual(['2:2', '3:1'])
      expect((await json(root, PAGE)).components[1].base.id).toBe(retired.id)
      expect(await pendingRetirements(root, records)).toEqual([])
      expect(await readFile(path.join(root, shared), 'utf8')).toContain('<svg')
      const before = await fixtureBytes(root)
      await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
      expect(await fixtureBytes(root)).toEqual(before)
    })
  })

  describe('and the retired component itself comes back at the top level', () => {
    beforeEach(async () => {
      await recapture((page) => [retired, ...page])
      await runSync(app, { source: 'snapshot', capturedPageIds: ['1:1'] })
    })

    it('should keep it out of generation without reporting the capture as pending', async () => {
      expect(await pendingRetirements(root, records)).toEqual([])
      await expect(readFile(path.join(root, GEN, 'page/First.tsx'))).rejects.toMatchObject({
        code: 'ENOENT'
      })
    })
  })

  describe('and a live component is merged into another set', () => {
    beforeEach(async () => {
      await recapture((page) => [successor('3:1', 'Merged', page[0]!)])
    })

    it('should still reject the identity move', async () => {
      expect((await guard()).violations.map((violation) => violation.cause)).toContainEqual(
        expect.stringMatching(
          /^artifactIdentity ".*Page\.json#variant:2:2" moved from .*"2:2".* to .*"3:1"/
        )
      )
    })
  })

  describe('and two live sets list the variant', () => {
    beforeEach(async () => {
      await recapture((page) => [
        ...page,
        successor('3:1', 'Merged', retired),
        successor('4:1', 'Other', retired, '4:9')
      ])
    })

    it('should reject the snapshot instead of resolving either set', async () => {
      expect((await guard()).violations.map((violation) => violation.cause)).toContainEqual(
        expect.stringContaining('duplicate entry variant:2:1')
      )
    })
  })

  describe('and the only set listing it is an external copy', () => {
    beforeEach(async () => {
      await recapture((page) => [
        ...page,
        { ...successor('3:1', 'Merged', retired), external: true }
      ])
    })

    it('should fail the guard without reporting the capture as pending', async () => {
      expect(await pendingRetirements(root, records)).toEqual([])
      expect((await guard()).ok).toBe(false)
    })
  })

  describe.each<[string, Partial<SavedComponent>]>([
    ['the only set listing it is truncated', { truncated: true }],
    ['the only component listing it is not a set', { kind: 'component', others: [] }]
  ])('and %s', (_, change) => {
    beforeEach(async () => {
      await recapture((page) => [...page, { ...successor('3:1', 'Merged', retired), ...change }])
    })

    it('should fail the guard without reporting the capture as pending', async () => {
      expect(await pendingRetirements(root, records)).toEqual([])
      expect((await guard()).ok).toBe(false)
    })
  })

  describe('and the successor is retired too', () => {
    let both: Retirements
    beforeEach(async () => {
      await recapture((page) => [...page, successor('3:1', 'Merged', retired)])
      both = {
        ...records,
        retired: [
          ...records.retired,
          {
            ...records.retired[0]!,
            componentId: '3:1',
            name: 'Merged',
            owned: [],
            sharedExclusions: []
          }
        ]
      }
    })

    it('should not report the capture as pending', async () => {
      expect(await pendingRetirements(root, both)).toEqual([])
    })
  })

  describe('and another saved page is unreadable', () => {
    beforeEach(async () => {
      await recapture((page) => [...page, successor('3:1', 'Merged', retired)])
      await put(root, `${SNAP}/components/Broken.json`, '{"page":')
    })

    it('should fail the guard without reporting the capture as pending', async () => {
      expect(await pendingRetirements(root, records)).toEqual([])
      expect((await guard()).ok).toBe(false)
    })
  })
})

describe('when a retirement is recorded before any recapture', () => {
  let root: string
  let app: App
  let records: Retirements
  let capture: Buffer
  beforeEach(async () => {
    ;({ root, app, records } = await lifecycleFixture())
    await put(root, `figma/ledger/retirements.json`, records)
    capture = await readFile(path.join(root, PAGE))
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  it('should report only outputs as pending, never the capture', async () => {
    const pending = await pendingRetirements(root, records)
    expect(pending.length).toBeGreaterThan(0)
    expect(pending.filter((artifact) => artifact.startsWith(`${SNAP}/`))).toEqual([])
  })

  describe('and the retirement is applied', () => {
    beforeEach(async () => {
      await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    })

    it('should clean the outputs', async () => {
      expect(await pendingRetirements(root, records)).toEqual([])
    })

    it('should leave the variant in the capture', async () => {
      expect(await readFile(path.join(root, PAGE))).toEqual(capture)
    })
  })
})

describe('reparentedTo', () => {
  const record = {
    pageId: '1:1',
    componentId: '2:1',
    name: 'First',
    reason: 'merged',
    owned: [],
    sharedExclusions: []
  }
  const retirements: Retirements = { schema: 1, retired: [record] }
  const set = { kind: 'set', truncated: false, external: false }
  const parents = (...owners: Partial<VariantParent>[]) => ({
    ...emptyOwnership(),
    variantParents: new Map([['2:1', owners.map((owner) => JSON.stringify({ ...set, ...owner }))]])
  })

  const live = { pageId: '1:1', componentId: '3:1' }
  describe.each<{ when: string; ownership: Ownership; expected: string | undefined }>([
    { when: 'no set lists the variant', ownership: emptyOwnership(), expected: undefined },
    { when: 'one live set on the same page lists it', ownership: parents(live), expected: '3:1' },
    {
      when: 'the only set is on another page',
      ownership: parents({ pageId: '1:2', componentId: '3:1' }),
      expected: undefined
    },
    {
      when: 'the retired component itself lists it',
      ownership: parents({ pageId: '1:1', componentId: '2:1' }),
      expected: undefined
    },
    {
      when: 'two sets list it',
      ownership: parents(live, { pageId: '1:1', componentId: '4:1' }),
      expected: undefined
    },
    {
      when: 'the only parent is a standalone component',
      ownership: parents({ ...live, kind: 'component' }),
      expected: undefined
    },
    {
      when: 'the only set is truncated',
      ownership: parents({ ...live, truncated: true }),
      expected: undefined
    },
    {
      when: 'the only set is an external copy',
      ownership: parents({ ...live, external: true }),
      expected: undefined
    },
    {
      when: 'a live set and an external copy list it',
      ownership: parents(live, { ...live, componentId: '9:1', external: true }),
      expected: undefined
    },
    {
      when: 'the snapshot has an unreadable page',
      ownership: { ...parents(live), conflicts: ['Invalid snapshot page identity'] },
      expected: undefined
    }
  ])('when $when', ({ ownership, expected }) => {
    it(`should resolve the parent to ${String(expected)}`, () => {
      expect(reparentedTo(ownership, retirements, record, '2:1')).toBe(expected)
    })
  })

  describe('when the retirement is page-level', () => {
    let page: Omit<typeof record, 'componentId'> & { componentId: undefined }
    beforeEach(() => {
      page = { ...record, componentId: undefined }
    })

    it('should not re-parent its variants', () => {
      expect(reparentedTo(parents(live), retirements, page, '2:1')).toBeUndefined()
    })
  })
})
