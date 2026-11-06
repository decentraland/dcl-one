import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  RETIREMENTS_PATH,
  emptyRetirements,
  filterRetired,
  isRetired,
  loadRetirements,
  serializeRetirements,
  type Retirements
} from './retirements.js'

const record = { name: 'x', reason: 'because', owned: [], sharedExclusions: [] }
const retirements: Retirements = {
  schema: 1,
  retired: [
    { ...record, pageId: '1:1' },
    { ...record, pageId: '2:2', componentId: '2:9' }
  ]
}

describe('retirements', () => {
  describe('when a page is retired', () => {
    it('should retire every component on it', () => {
      expect(isRetired(retirements, '1:1')).toBe(true)
      expect(isRetired(retirements, '1:1', '1:7')).toBe(true)
    })
  })

  describe('when only a component is retired', () => {
    it('should retire that component and nothing else on the page', () => {
      expect(isRetired(retirements, '2:2', '2:9')).toBe(true)
      expect(isRetired(retirements, '2:2', '2:10')).toBe(false)
      expect(isRetired(retirements, '2:2')).toBe(false)
    })
  })

  describe('when nothing is retired', () => {
    it('should keep every item', () => {
      const items = [{ pageId: '3:3' }, { pageId: '1:1', componentId: '1:7' }]

      expect(filterRetired(emptyRetirements(), items)).toEqual({ kept: items, retired: [] })
    })
  })

  describe('when filtering a mixed list', () => {
    it('should split retired from kept', () => {
      const items = [{ pageId: '3:3' }, { pageId: '1:1', componentId: '1:7' }]

      const { kept, retired } = filterRetired(retirements, items)

      expect(kept).toEqual([{ pageId: '3:3' }])
      expect(retired).toEqual([{ pageId: '1:1', componentId: '1:7' }])
    })
  })

  describe('when loading the file', () => {
    let root: string
    beforeEach(async () => {
      root = await mkdtemp(path.join(os.tmpdir(), 'dcl-figma-retire-'))
    })
    afterEach(async () => {
      await rm(root, { recursive: true, force: true })
    })

    it('should return an empty record when the file is missing', async () => {
      expect((await loadRetirements(root)).retired).toEqual([])
    })

    it('should parse a valid file', async () => {
      await mkdir(path.dirname(path.join(root, RETIREMENTS_PATH)), { recursive: true })
      await writeFile(path.join(root, RETIREMENTS_PATH), JSON.stringify(retirements))

      expect((await loadRetirements(root)).retired).toHaveLength(2)
    })

    it.each(['ledger', 'record'])('should reject unknown fields in the %s', async (scope) => {
      const data = structuredClone(retirements)
      Object.assign(scope === 'ledger' ? data : data.retired[0]!, { unexpected: true })
      await mkdir(path.dirname(path.join(root, RETIREMENTS_PATH)), { recursive: true })
      await writeFile(path.join(root, RETIREMENTS_PATH), JSON.stringify(data))
      await expect(loadRetirements(root)).rejects.toThrow('Unrecognized key')
    })

    it('should serialize equivalent ledgers identically without mutating the input', () => {
      const data = structuredClone(retirements)
      data.retired.reverse()
      data.retired[0]!.owned = ['z.tsx', 'a.json#component:2:9']
      data.retired[0]!.sharedExclusions = ['z.svg', 'a.svg']
      const before = structuredClone(data)
      const canonical = JSON.parse(serializeRetirements(data)) as Retirements
      expect(canonical.retired.map((entry) => entry.pageId)).toEqual(['1:1', '2:2'])
      expect(canonical.retired[1]!.owned).toEqual(['a.json#component:2:9', 'z.tsx'])
      expect(canonical.retired[1]!.sharedExclusions).toEqual(['a.svg', 'z.svg'])
      expect(serializeRetirements(canonical)).toBe(serializeRetirements(data))
      expect(data).toEqual(before)
    })

    it('should reject a malformed file instead of treating it as empty', async () => {
      await mkdir(path.dirname(path.join(root, RETIREMENTS_PATH)), { recursive: true })
      await writeFile(path.join(root, RETIREMENTS_PATH), '{"schema":2}')

      await expect(loadRetirements(root)).rejects.toThrow()
    })
  })
})
