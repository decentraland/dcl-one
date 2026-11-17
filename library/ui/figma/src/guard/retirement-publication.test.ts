import { readFile, rm } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { App } from '../app.js'
import { runSync, runDiff } from '../app.js'
import * as edits from './aggregate-edits.js'
import * as cleanup from './apply-retirements.js'
import { lifecycleFixture, fixtureBytes, put, json, SNAP, GEN, LIB } from './lifecycle-fixture.js'
import type { Retirements } from './retirements.js'

const options = { source: 'snapshot', developer: true, applyRetirements: true } as const
describe('when retirement publication is interrupted', () => {
  let root: string
  let app: App
  let records: Retirements
  let shared: string
  beforeEach(async () => {
    ;({ root, app, records, shared } = await lifecycleFixture())
    await put(root, `figma/ledger/retirements.json`, records)
  })
  afterEach(async () => {
    vi.restoreAllMocks()
    await rm(root, { recursive: true, force: true })
  })
  it.each(['ordinary entries', 'manifest entries'] as const)(
    'should retry after removing %s while retaining source identity anchors',
    async (phase) => {
      const remove = edits.removeAggregateEntries
      const interrupt = vi
        .spyOn(edits, 'removeAggregateEntries')
        .mockImplementation(async (directory, artifacts) => {
          const result = await remove(directory, artifacts)
          const manifestPhase = artifacts.some((artifact) =>
            artifact.startsWith(`${GEN}/manifest.ts#`)
          )
          if ((phase === 'manifest entries') === manifestPhase)
            throw new Error('Interrupted metadata cleanup')
          return result
        })
      const before = await readFile(path.join(root, shared), 'utf8')
      await expect(cleanup.applyRetirements(root, records)).rejects.toThrow(
        'Interrupted metadata cleanup'
      )
      interrupt.mockRestore()
      expect((await json(root, `${SNAP}/components/Page.json`)).components).toHaveLength(2)
      await cleanup.applyRetirements(root, records)
      expect(await cleanup.pendingRetirements(root, records)).toEqual([])
      expect(await json(root, `figma/ledger/retirements.json`)).toEqual(records)
      expect(await readFile(path.join(root, shared), 'utf8')).toBe(before)
      expect(
        (await json(root, `${SNAP}/components/Page.json`)).components.map(
          (component: { id: string }) => component.id
        )
      ).toEqual(['2:1', '2:2'])
    }
  )
  it('should apply a validated reason edit while publication is pending', async () => {
    const apply = cleanup.applyRetirements
    const interrupt = vi
      .spyOn(cleanup, 'applyRetirements')
      .mockImplementation(async (directory, retirement) => {
        if (directory === root) throw new Error('Interrupted publication')
        return apply(directory, retirement)
      })
    await expect(runSync(app, options)).rejects.toThrow('Interrupted publication')
    interrupt.mockRestore()
    records.retired[0]!.reason = 'Changed while pending'
    await put(root, `figma/ledger/retirements.json`, records)
    await runSync(app, options)
    expect(await json(root, `figma/ledger/retirements.json`)).toEqual(records)
    expect(await cleanup.pendingRetirements(root, records)).toEqual([])
  })
  it('should retire a shared asset only when its last user is explicitly retired', async () => {
    await runSync(app, options)
    records.retired.push({
      ...records.retired[0]!,
      componentId: '2:2',
      name: 'Second',
      ...(await cleanup.retirementArtifacts(root, { pageId: '1:1', componentId: '2:2' }))
    })
    await put(root, `figma/ledger/retirements.json`, records)
    await runSync(app, options)
    await expect(readFile(path.join(root, shared))).rejects.toMatchObject({ code: 'ENOENT' })
    await expect(readFile(path.join(root, shared.replace(GEN, LIB)))).rejects.toMatchObject({
      code: 'ENOENT'
    })
    expect(await json(root, `figma/ledger/retirements.json`)).toEqual(records)
    expect(records.retired.map((record) => record.componentId)).toEqual(['2:1', '2:2'])
    expect((await runDiff(app, { source: 'snapshot' })).diff).toEqual({
      added: [],
      changed: [],
      removed: []
    })
    const before = await fixtureBytes(root)
    expect((await runSync(app, options)).retired).toEqual([])
    expect(await fixtureBytes(root)).toEqual(before)
  })
})
