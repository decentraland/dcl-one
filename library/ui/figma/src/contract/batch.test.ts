import { readdir, rm } from 'node:fs/promises'
import { afterEach, describe, expect, it } from 'vitest'
import { resolveBatch } from './batch.js'
import { buttonSet, makeLibrary, mappingSource } from './test-fixtures.js'

const created: string[] = []

afterEach(async () => {
  await Promise.all(created.splice(0).map((dir) => rm(dir, { recursive: true, force: true })))
})

async function library() {
  const made = await makeLibrary({
    mappings: {
      'Button.figma.ts': mappingSource(),
      'Card.figma.ts': mappingSource({ nodeId: '3-3', component: 'Card', id: 'card' })
    }
  })
  created.push(made.root)
  return made
}

describe('resolveBatch', () => {
  describe('when the batch mixes resolvable, unmapped and unknown entries', () => {
    it('should answer every entry in input order and count them', async () => {
      const { root, mappingRoot } = await library()
      const before = (await readdir(root, { recursive: true })).sort()
      const unmapped = { ...buttonSet, setId: '5:5', name: 'Badge', variants: [] }

      const batch = await resolveBatch({ mappingRoot, figmaSets: [buttonSet, unmapped] }, [
        { nodeId: '1:1', variantProperties: { Kind: 'Secondary' } },
        { name: 'Badge' },
        { nodeId: '9:9' },
        { name: 'Button', variantProperties: { Kind: 'Primary' }, texts: { Label: 'Go' } }
      ])

      expect(batch.ok).toBe(false)
      if (!('results' in batch)) return
      expect(batch.results.map((entry) => entry.index)).toEqual([0, 1, 2, 3])
      expect(batch.results.map((entry) => entry.result.ok)).toEqual([true, false, false, true])
      expect(batch.summary).toEqual({
        requested: 4,
        resolved: 2,
        unmapped: [{ index: 1, name: 'Badge' }],
        failed: [{ index: 2, code: 'identity-not-found' }]
      })
      // Read-only: nothing is written next to the mappings.
      expect((await readdir(root, { recursive: true })).sort()).toEqual(before)
    })
  })

  describe('when every entry resolves without a capture', () => {
    it('should pass', async () => {
      const { mappingRoot } = await library()

      const batch = await resolveBatch({ mappingRoot, figmaSets: [] }, [
        { name: 'Button', variantProperties: { Kind: 'Primary' } },
        { name: 'button', variantProperties: { Kind: 'Secondary' } }
      ])

      expect(batch).toMatchObject({
        ok: true,
        summary: { requested: 2, resolved: 2, unmapped: [], failed: [] }
      })
    })
  })

  describe.each([
    ['an empty array', []],
    ['an object', { nodeId: '1:1' }],
    ['a string', '1:1'],
    ['null', null]
  ])('when the batch is %s', (_label, entries) => {
    it('should refuse the whole batch with invalid-input', async () => {
      const { mappingRoot } = await library()

      expect(await resolveBatch({ mappingRoot, figmaSets: [] }, entries)).toMatchObject({
        ok: false,
        errors: [expect.objectContaining({ code: 'invalid-input' })]
      })
    })
  })

  describe.each([
    ['an unknown field', { nodeId: '1:1', componentKey: 'fake-key' }],
    ['a non-string variant value', { name: 'Button', variantProperties: { Kind: 1 } }],
    ['an empty name', { name: '' }],
    ['a non-object entry', 'Button'],
    ['neither id nor name', { variantProperties: { Kind: 'Primary' } }]
  ])('when an entry has %s', (_label, entry) => {
    it('should fail only that entry with invalid-input', async () => {
      const { mappingRoot } = await library()

      const batch = await resolveBatch({ mappingRoot, figmaSets: [buttonSet] }, [
        { nodeId: '1:1' },
        entry
      ])

      expect(batch).toMatchObject({
        ok: false,
        summary: { requested: 2, resolved: 1, failed: [{ index: 1, code: 'invalid-input' }] }
      })
    })
  })

  describe('when an entry repeats an earlier one', () => {
    it('should refuse the repeat and name the first entry', async () => {
      const { mappingRoot } = await library()

      const batch = await resolveBatch({ mappingRoot, figmaSets: [buttonSet] }, [
        { nodeId: '1:1', variantProperties: { Kind: 'Primary', Blocked: 'No' } },
        { variantProperties: { Blocked: 'No', Kind: 'Primary' }, nodeId: '1:1' }
      ])

      if (!('results' in batch)) throw new Error('expected per-entry results')
      expect(batch.ok).toBe(true)
      expect(batch.results[1]).toMatchObject({ repeats: 0, result: { ok: true } })
      expect(batch.summary).toMatchObject({ requested: 2, resolved: 2, failed: [] })
    })

    it('should treat 1-1 and 1:1 as the same node', async () => {
      const { mappingRoot } = await library()

      const batch = await resolveBatch({ mappingRoot, figmaSets: [buttonSet] }, [
        { nodeId: '1:1' },
        { nodeId: '1-1' }
      ])

      expect('results' in batch && batch.results[1]!.repeats).toBe(0)
    })
  })

  describe('when a refused entry carries a field the batch does not accept', () => {
    it('should echo the field name, never its value', async () => {
      const { mappingRoot } = await library()

      const batch = await resolveBatch({ mappingRoot, figmaSets: [] }, [
        { name: 'Button', componentKey: 'fake-component-key-value' }
      ])

      expect(JSON.stringify(batch)).not.toContain('fake-component-key-value')
      expect('results' in batch && batch.results[0]!.input).toEqual({
        name: 'Button',
        unknownFields: ['componentKey']
      })
    })
  })
})
