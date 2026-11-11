import { cp, mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createApp } from '../app.js'
import { writeReviewManifest } from '../review-manifest.js'
import { checkScope } from './check-scope.js'
import { SNAP, json, lifecycleFixture, put } from './lifecycle-fixture.js'
import { emptyRetirements } from './retirements.js'

const CATALOG = 'figma/review/src/review/catalog.json'

describe('when a captured component changes its stroke weight', () => {
  let base: string
  let staged: string

  beforeEach(async () => {
    base = (await lifecycleFixture()).root
    staged = await mkdtemp(path.join(os.tmpdir(), 'derived-metadata-staged-'))
    await cp(base, staged, { recursive: true })
    const page = await json(staged, `${SNAP}/components/Page.json`)
    page.components.find(
      (component: { name: string }) => component.name === 'First'
    ).base.tree.strokeWeight = 3
    await put(staged, `${SNAP}/components/Page.json`, page)
    await writeReviewManifest(
      await createApp(path.join(staged, 'figma'), undefined, { unverifiedSnapshot: true })
    )
  })

  afterEach(async () => {
    await rm(base, { recursive: true, force: true })
    await rm(staged, { recursive: true, force: true })
  })

  const check = () =>
    checkScope({
      baseDir: base,
      stagedDir: staged,
      capturedPageIds: ['1:1'],
      runKind: 'automated',
      stage: 'complete',
      retirements: emptyRetirements()
    })

  it('should accept foundations.strokeWidths recomputed over the full inventory', async () => {
    expect((await json(base, CATALOG)).foundations.strokeWidths).toEqual([])
    expect((await json(staged, CATALOG)).foundations.strokeWidths).toEqual([3])

    expect(await check()).toEqual({ ok: true, violations: [] })
  })

  it('should reject foundations.strokeWidths that differ from the recomputation', async () => {
    const catalog = await json(staged, CATALOG)
    catalog.foundations.strokeWidths = [3, 5]
    await put(staged, CATALOG, catalog)

    expect(await check()).toEqual({
      ok: false,
      violations: [
        expect.objectContaining({
          path: CATALOG,
          kind: 'aggregate-entry-changed',
          cause:
            'foundations.strokeWidths does not match its recomputation over the full inventory',
          metadataChanges: [{ field: '/foundations/strokeWidths', before: [], after: [3, 5] }]
        })
      ]
    })
  })

  it('should reject any other non-entry change the recomputation does not explain', async () => {
    const catalog = await json(staged, CATALOG)
    catalog.foundations.effects.status = 'Edited outside a foundations capture'
    await put(staged, CATALOG, catalog)

    expect(await check()).toEqual({
      ok: false,
      violations: [
        expect.objectContaining({
          path: CATALOG,
          kind: 'aggregate-entry-changed',
          cause: 'non-entry part of the aggregate changed',
          metadataChanges: expect.arrayContaining([
            {
              field: '/foundations/effects/status',
              before: 'Disabled in Figma. Existing captured effects are frozen.',
              after: 'Edited outside a foundations capture'
            }
          ])
        })
      ]
    })
  })

  describe('and the staged inventory cannot be read', () => {
    beforeEach(async () => {
      await rm(path.join(staged, 'figma/figma.config.json'))
    })

    it('should retain the metadata diff alongside the recomputation failure', async () => {
      expect((await check()).violations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            path: CATALOG,
            cause: expect.stringContaining(
              'foundations.strokeWidths changed but could not be recomputed'
            ),
            metadataChanges: [{ field: '/foundations/strokeWidths', before: [], after: [3] }]
          })
        ])
      )
    })
  })
})
