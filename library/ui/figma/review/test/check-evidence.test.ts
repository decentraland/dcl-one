import { describe, it, beforeEach, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { checkEvidence } from '../scripts/check-evidence.mjs'
import { preserveFrozenCases, staleCases } from '../scripts/capture-scope.mjs'

// A Review with two components, each with one captured variant.
const OLD = 'old-signature'
const CURRENT = 'current-signature'
const catalog = {
  components: [
    { name: 'A', page: 'Page A', variants: [{ id: '1:1' }] },
    { name: 'B', page: 'Page B', variants: [{ id: '2:2' }] }
  ]
}
const evidence = (key: string, sourceId: string) => ({
  sourceId,
  width: 10,
  height: 10,
  image: `/dcl-ui-review/renders/${key}.png`,
  status: 'exact'
})

let root: string

async function write(report: unknown) {
  await mkdir(path.join(root, 'src/review'), { recursive: true })
  await mkdir(path.join(root, 'public/dcl-ui-review/renders'), { recursive: true })
  await writeFile(path.join(root, 'src/review/catalog.json'), JSON.stringify(catalog))
  for (const key of ['A-0', 'B-0'])
    await writeFile(path.join(root, `public/dcl-ui-review/renders/${key}.png`), 'png')
  await writeFile(path.join(root, 'public/dcl-ui-review/fidelity.json'), JSON.stringify(report))
}

beforeEach(async () => {
  root = await mkdtemp(path.join(os.tmpdir(), 'review-evidence-'))
})
afterEach(async () => {
  await rm(root, { recursive: true, force: true })
})

describe('when checking the visual evidence against the current source', () => {
  it('should accept evidence a full capture signed with the current signature', async () => {
    await write({
      signature: CURRENT,
      cases: { 'A-0': evidence('A-0', '1:1'), 'B-0': evidence('B-0', '2:2') }
    })

    await checkEvidence({ root, signature: CURRENT })
  })

  it('should reject a report signed with another signature', async () => {
    await write({
      signature: OLD,
      cases: { 'A-0': evidence('A-0', '1:1'), 'B-0': evidence('B-0', '2:2') }
    })

    await assert.rejects(checkEvidence({ root, signature: CURRENT }), /Visual evidence is stale\./)
  })

  it('should reject a variant that has no evidence', async () => {
    await write({ signature: CURRENT, cases: { 'A-0': evidence('A-0', '1:1') } })

    await assert.rejects(checkEvidence({ root, signature: CURRENT }), /incomplete: B/)
  })

  describe('and a partial capture refreshed only one page', () => {
    // What capture-review writes: the kept page from the previous report, the refreshed page fresh, and the
    // report signed with the current signature.
    const partial = (previous: { signature: string; cases: Record<string, unknown> }) => ({
      signature: CURRENT,
      cases: {
        ...preserveFrozenCases([catalog.components[0]], previous),
        'B-0': evidence('B-0', '2:2')
      }
    })

    it('should reject the kept page, which was captured from an older source', async () => {
      await write(
        partial({
          signature: OLD,
          cases: { 'A-0': evidence('A-0', '1:1'), 'B-0': evidence('B-0', '2:2') }
        })
      )

      await assert.rejects(
        checkEvidence({ root, signature: CURRENT }),
        /stale for 1 case\(s\) kept from an earlier capture \(A-0\)/
      )
    })

    it('should still reject it after a second partial capture that kept the same page', async () => {
      const first = partial({
        signature: OLD,
        cases: { 'A-0': evidence('A-0', '1:1'), 'B-0': evidence('B-0', '2:2') }
      })
      await write({ ...partial(first), signature: CURRENT })

      await assert.rejects(
        checkEvidence({ root, signature: CURRENT }),
        /kept from an earlier capture \(A-0\)/
      )
    })

    it('should accept it when the source did not change since the kept page was captured', async () => {
      await write(
        partial({
          signature: CURRENT,
          cases: { 'A-0': evidence('A-0', '1:1'), 'B-0': evidence('B-0', '2:2') }
        })
      )

      await checkEvidence({ root, signature: CURRENT })
    })
  })
})

describe('when listing the cases captured under another signature', () => {
  it('should treat a case without its own signature as the report’s', () => {
    const report = { signature: CURRENT, cases: { 'A-0': { sourceId: '1:1' } } }

    assert.deepEqual(staleCases(report, CURRENT), [])
    assert.deepEqual(staleCases(report, OLD), ['A-0'])
  })

  it('should name only the cases that carry another signature', () => {
    const report = {
      signature: CURRENT,
      cases: { 'A-0': { sourceId: '1:1', signature: OLD }, 'B-0': { sourceId: '2:2' } }
    }

    assert.deepEqual(staleCases(report, CURRENT), ['A-0'])
  })

  it('should keep the signature a frozen case already carries', () => {
    const kept = preserveFrozenCases([catalog.components[0]], {
      signature: CURRENT,
      cases: { 'A-0': { ...evidence('A-0', '1:1'), signature: OLD } }
    })

    assert.equal((kept['A-0'] as { signature: string }).signature, OLD)
  })
})
