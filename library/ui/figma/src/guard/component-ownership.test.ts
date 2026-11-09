import { readFile, rm } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { App } from '../app.js'
import { runSync } from '../app.js'
import { lifecycleFixture, put, json, SNAP, GEN, LIB } from './lifecycle-fixture.js'
import { buildOwnership, ownerOf, type Ownership } from './ownership.js'
import { retirementArtifacts } from './apply-retirements.js'
import type { Retirements } from './retirements.js'

describe('when resolving component and shared-asset ownership', () => {
  let root: string
  let app: App
  let shared: string
  let records: Retirements
  let ownership: Ownership
  beforeEach(async () => {
    ;({ root, app, shared, records } = await lifecycleFixture())
    ownership = await buildOwnership(root)
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should identify the same component across modules, mappings, snapshot entries and public exports', () => {
    for (const file of [
      `${GEN}/page/First.tsx`,
      `${GEN}/page/First.module.css`,
      `${LIB}/page/First.figma.ts`,
      `${SNAP}/components/Page.json#component:2:1`,
      `${SNAP}/components/Page.json#variant:2:1`,
      `${GEN}/manifest.ts#component:First`,
      'figma/review/src/review/catalog.json#component:First',
      'index.ts#value:First',
      'index.ts#type:FirstProps',
      'index.ts#type:FirstVariant'
    ])
      expect(ownerOf(ownership, file)).toEqual({ pageId: '1:1', componentId: '2:1' })
    expect(ownerOf(ownership, `${GEN}/page/Second.tsx`)).toEqual({
      pageId: '1:1',
      componentId: '2:2'
    })
  })
  it('should retain the complete user set for one asset shared by sibling components', () => {
    for (const asset of [shared, shared.replace(GEN, LIB)]) {
      expect(ownerOf(ownership, asset)).toEqual({
        aggregate: 'assets',
        users: expect.arrayContaining([
          { pageId: '1:1', componentId: '2:1' },
          { pageId: '1:1', componentId: '2:2' }
        ]),
        foreignUsers: []
      })
      expect((ownerOf(ownership, asset) as { users: unknown[] }).users).toHaveLength(2)
    }
  })
  it('should remove only the retired user from shared-asset ownership after cleanup', async () => {
    await put(root, `figma/ledger/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    ownership = await buildOwnership(root)
    expect(ownerOf(ownership, shared)).toEqual({
      aggregate: 'assets',
      users: [{ pageId: '1:1', componentId: '2:2' }],
      foreignUsers: []
    })
    expect(await readFile(path.join(root, shared), 'utf8')).toContain('<svg')
  })
  it('should preserve foreign module-prefix files during component retirement', async () => {
    const foreign = [`${GEN}/page/First.custom.tsx`, `${LIB}/page/First.custom.tsx`]
    for (const file of foreign) await put(root, file, 'foreign')
    ownership = await buildOwnership(root)
    const artifacts = await retirementArtifacts(root, { pageId: '1:1', componentId: '2:1' })
    for (const file of foreign) {
      expect(ownerOf(ownership, file)).toBeNull()
      expect(artifacts.owned).not.toContain(file)
    }
  })
  it('should exclude shared assets consumed by foreign code from page retirement', async () => {
    const foreign = 'figma/review/src/custom/Keep.tsx'
    await put(
      root,
      foreign,
      `export const artwork = '../generated/components/assets/${path.basename(shared)}'`
    )
    ownership = await buildOwnership(root)
    expect(ownerOf(ownership, shared)).toMatchObject({ foreignUsers: [foreign] })
    const artifacts = await retirementArtifacts(root, { pageId: '1:1' })
    expect(artifacts.owned).not.toContain(shared)
    expect(artifacts.sharedExclusions).toContain(shared)
  })
  it('should never assign a published sibling to an unpublished component sharing its name', async () => {
    const page = await json(root, `${SNAP}/components/Page.json`)
    page.components.push({
      ...page.components[0],
      id: '9:9',
      base: { ...page.components[0].base, id: '9:9' }
    })
    await put(root, `${SNAP}/components/Page.json`, page)
    ownership = await buildOwnership(root)
    expect(ownerOf(ownership, `${SNAP}/components/Page.json#component:9:9`)).toEqual({
      pageId: '1:1',
      componentId: '9:9'
    })
    expect(ownerOf(ownership, `${GEN}/page/First.tsx`)).toEqual({
      pageId: '1:1',
      componentId: '2:1'
    })
    const artifacts = await retirementArtifacts(root, { pageId: '1:1', componentId: '9:9' })
    expect(artifacts.owned).toContain(`${SNAP}/components/Page.json#component:9:9`)
    expect(artifacts.owned).not.toContain(`${GEN}/page/First.tsx`)
    expect(artifacts.owned).not.toContain('index.ts#value:First')
  })
})
