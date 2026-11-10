import { cp, mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { runSync, type App } from '../app.js'
import { checkScope } from './check-scope.js'
import { lifecycleFixture, put, json } from './lifecycle-fixture.js'
import { emptyRetirements, type Retirements } from './retirements.js'

describe('when the general PR guard checks library promotion entries', () => {
  let base: string
  let root: string
  let app: App
  let records: Retirements
  beforeEach(async () => {
    ;({ root, app, records } = await lifecycleFixture())
    base = await mkdtemp(path.join(os.tmpdir(), 'dcl-library-guard-'))
    await cp(root, base, { recursive: true })
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
    await rm(base, { recursive: true, force: true })
  })
  const check = (retirements: Retirements) =>
    checkScope({
      baseDir: base,
      stagedDir: root,
      capturedPageIds: [],
      runKind: 'developer',
      stage: 'complete',
      retirements,
      removalsOnly: ['components']
    })
  it('should reject component removal from only the promotion manifest without authorization', async () => {
    await put(root, 'components/promotion.json', {
      ...(await json(root, 'components/promotion.json')),
      components: []
    })
    expect(await check(emptyRetirements())).toMatchObject({
      ok: false,
      violations: expect.arrayContaining([
        expect.objectContaining({ path: 'components/promotion.json', kind: 'removed' })
      ])
    })
  })
  it('should accept component removal authorized by a permanent retirement', async () => {
    await put(root, `figma/ledger/retirements.json`, records)
    await runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    expect(await check(records)).toEqual({ ok: true, violations: [] })
  })
})
