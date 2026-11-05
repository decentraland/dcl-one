import { describe, expect, it } from 'vitest'
import { emptyRetirements, retirementsSchema, type Retirements } from './guard/retirements.js'
import { sourceRetirements } from './retirement-selection.js'

// Keyless by construction: the tool works on the one file FIGMA_FILE_KEY names, so retirements carry no
// file key and are never checked against one.
describe('when loading retirement identities', () => {
  const records: Retirements = {
    schema: 1,
    retired: [
      {
        pageId: 'page',
        name: 'Retired',
        reason: 'Explicit removal',
        owned: [],
        sharedExclusions: []
      }
    ]
  }

  it('should read an absent retirement source as no retirements', async () => {
    expect(await sourceRetirements({})).toEqual(emptyRetirements())
  })

  it('should return the saved records as they are', async () => {
    expect(await sourceRetirements({ getRetirements: async () => records })).toEqual(records)
  })

  describe('and a saved record carries a file key', () => {
    it('should reject it, so no key can be stored in retirements', () => {
      const planted = {
        ...records,
        retired: [{ ...records.retired[0]!, fileKey: 'FAKEKEY000example000000' }]
      }

      expect(() => retirementsSchema.parse(planted)).toThrow()
      expect(() => retirementsSchema.parse({ ...records, fileKey: 'x' })).toThrow()
    })
  })
})
