import * as reviewManifest from '../review-manifest.js'
import { cp, mkdtemp, readFile, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, runSync, runDiff, type App } from '../app.js'
import { computeOwned, pendingRetirements } from './apply-retirements.js'
import type { Retirements } from './retirements.js'
import { lifecycleFixture, fixtureBytes, put, json, SNAP, GEN, LIB } from './lifecycle-fixture.js'

describe('when applying a component retirement in a retained page', () => {
  let root: string
  let app: App
  let records: Retirements
  let shared: string
  beforeEach(async () => {
    ;({ root, app, shared, records } = await lifecycleFixture())
  })
  afterEach(async () => {
    vi.restoreAllMocks()
    await rm(root, { recursive: true, force: true })
  })
  it('should record component entries without claiming the sibling or shared container', () => {
    expect(records.retired[0]!.owned).not.toContain(`${SNAP}/components/Page.json`)
    expect(records.retired[0]!.owned).toContain(`${SNAP}/components/Page.json#component:2:1`)
    expect(records.retired[0]!.owned).not.toContain(`${GEN}/page/Second.tsx`)
    expect(records.retired[0]!.owned).not.toContain(
      'figma/review/public/dcl-ui-review/references/live-2-2.png'
    )
    expect(records.retired[0]!.owned).not.toContain(shared)
  })
  it('should clean the exact component and all its value and type exports', async () => {
    await put(root, `${SNAP}/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    expect(
      (await json(root, `${SNAP}/components/Page.json`)).components.map(
        (component: { id: string }) => component.id
      )
    ).toEqual(['2:2'])
    expect(
      (await json(root, 'figma/review/src/review/catalog.json')).components.map(
        (component: { name: string }) => component.name
      )
    ).toEqual(['Second'])
    expect(await readFile(path.join(root, shared), 'utf8')).toContain('<svg')
    expect(await readFile(path.join(root, shared.replace(GEN, LIB)), 'utf8')).toContain('<svg')
    expect(await readFile(path.join(root, 'index.ts'), 'utf8')).not.toContain('First')
    expect(await readFile(path.join(root, `${LIB}/page/Second.tsx`), 'utf8')).toContain('Second')
    expect((await json(root, 'components/promotion.json')).components).toEqual(['Second'])
    await expect(readFile(path.join(root, `${LIB}/page/First.figma.ts`))).rejects.toMatchObject({
      code: 'ENOENT'
    })
  })
  it('should be a no-op on repeated application', async () => {
    await put(root, `${SNAP}/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    expect(
      (await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })).retired
    ).toEqual([])
  })
  it('should retain the permanent record across an interrupted cleanup and retry', async () => {
    await put(root, `${SNAP}/retirements.json`, records)
    const paths = [
      `${SNAP}/retirements.json`,
      `${SNAP}/components/Page.json`,
      `${GEN}/page/First.tsx`,
      `${LIB}/page/First.tsx`,
      'index.ts',
      'figma/review/src/review/catalog.json'
    ]
    const before = await Promise.all(paths.map((file) => readFile(path.join(root, file), 'utf8')))
    const interruption = vi
      .spyOn(reviewManifest, 'writeReviewManifest')
      .mockRejectedValueOnce(new Error('Interrupted staged cleanup'))
    await expect(
      runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    ).rejects.toThrow('Interrupted staged cleanup')
    interruption.mockRestore()
    expect(await Promise.all(paths.map((file) => readFile(path.join(root, file), 'utf8')))).toEqual(
      before
    )
    expect((await pendingRetirements(root, records)).length).toBeGreaterThan(0)

    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    expect(await json(root, `${SNAP}/retirements.json`)).toEqual(records)
    expect(await pendingRetirements(root, records)).toEqual([])
    expect(
      (await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })).retired
    ).toEqual([])
  })
  it('should resume a partial cleanup without losing its permanent identity or shared assets', async () => {
    await put(root, `${SNAP}/retirements.json`, records)
    const sharedBefore = await readFile(path.join(root, shared), 'utf8')
    await rm(path.join(root, `${GEN}/page/First.tsx`))
    await rm(path.join(root, `${LIB}/page/First.figma.ts`))
    expect((await pendingRetirements(root, records)).length).toBeGreaterThan(0)

    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    expect(await json(root, `${SNAP}/retirements.json`)).toEqual(records)
    expect(await pendingRetirements(root, records)).toEqual([])
    expect(await readFile(path.join(root, shared), 'utf8')).toBe(sharedBefore)
    expect(
      (await json(root, `${SNAP}/components/Page.json`)).components.map(
        (component: { id: string }) => component.id
      )
    ).toEqual(['2:2'])
  })
  it('should apply the same committed record from a clean checkout', async () => {
    await put(root, `${SNAP}/retirements.json`, records)
    const copy = await mkdtemp(path.join(os.tmpdir(), 'retirement-checkout-'))
    try {
      await cp(root, copy, { recursive: true })
      await runSync(
        await createApp(path.join(copy, 'figma'), undefined, { unverifiedSnapshot: true }),
        {
          source: 'snapshot',
          developer: true,
          applyRetirements: true
        }
      )
      expect(
        (await json(copy, `${SNAP}/components/Page.json`)).components.map(
          (component: { id: string }) => component.id
        )
      ).toEqual(['2:2'])
    } finally {
      await rm(copy, { recursive: true, force: true })
    }
  })
  it('should retire an entire page without leaving drift or shared artifacts behind', async () => {
    records.retired = [
      {
        ...records.retired[0]!,
        componentId: undefined,
        name: 'Page',
        owned: await computeOwned(root, { pageId: '1:1' }),
        sharedExclusions: []
      }
    ]
    await put(root, `${SNAP}/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    await expect(readFile(path.join(root, `${SNAP}/components/Page.json`))).rejects.toMatchObject({
      code: 'ENOENT'
    })
    expect((await json(root, `${SNAP}/generation-inventory.json`)).review).toEqual([])
    expect((await json(root, 'figma/review/src/review/catalog.json')).components).toEqual([])
    expect((await runDiff(app, { source: 'snapshot' })).diff).toEqual({
      added: [],
      changed: [],
      removed: []
    })
  })
  it('should reject a forged sibling claim before changing any published file', async () => {
    records.retired[0]!.owned.push(`${GEN}/page/Second.tsx`)
    await put(root, `${SNAP}/retirements.json`, records)
    const before = await readFile(path.join(root, `${GEN}/page/First.tsx`), 'utf8')
    await expect(
      runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    ).rejects.toThrow('not owned')
    expect(await readFile(path.join(root, `${GEN}/page/First.tsx`), 'utf8')).toBe(before)
    expect((await json(root, `${SNAP}/components/Page.json`)).components).toHaveLength(2)
  })
})

describe('when Foo retires while its duplicate Foo2 survives', () => {
  let root: string
  let app: App
  let records: Retirements
  let shared: string
  beforeEach(async () => {
    ;({ root, app, records, shared } = await lifecycleFixture(false, true))
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should reserve Foo permanently and keep Foo2 bytes identical across cleanup and a no-op retry', async () => {
    const siblingFiles = [
      `${GEN}/page/Foo2.tsx`,
      `${GEN}/page/Foo2.module.css`,
      `${LIB}/page/Foo2.tsx`,
      `${LIB}/page/Foo2.module.css`,
      `${LIB}/page/Foo2.figma.ts`
    ]
    const before = await Promise.all(siblingFiles.map((file) => readFile(path.join(root, file))))
    const sharedBefore = await readFile(path.join(root, shared))
    await put(root, `${SNAP}/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    expect(
      (await json(root, 'figma/review/src/review/catalog.json')).components.map(
        (component: { id: string; name: string }) => [component.id, component.name]
      )
    ).toEqual([['2:2', 'Foo2']])
    expect(await Promise.all(siblingFiles.map((file) => readFile(path.join(root, file))))).toEqual(
      before
    )
    expect(await readFile(path.join(root, shared))).toEqual(sharedBefore)
    await expect(readFile(path.join(root, `${GEN}/page/Foo.tsx`))).rejects.toMatchObject({
      code: 'ENOENT'
    })
    expect(await json(root, `${SNAP}/retirements.json`)).toEqual(records)
    expect((await runDiff(app, { source: 'snapshot' })).diff).toEqual({
      added: [],
      changed: [],
      removed: []
    })
    const afterCleanup = await fixtureBytes(root)
    expect(
      (await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })).retired
    ).toEqual([])
    expect(await fixtureBytes(root)).toEqual(afterCleanup)
    expect(await json(root, `${SNAP}/retirements.json`)).toEqual(records)
  })
})
