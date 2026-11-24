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

const writeLedger = async (root: string, contents: string) => {
  await mkdir(path.dirname(path.join(root, RETIREMENTS_PATH)), { recursive: true })
  await writeFile(path.join(root, RETIREMENTS_PATH), contents)
}

describe('retirements', () => {
  let retirements: Retirements

  beforeEach(() => {
    const record = { name: 'x', reason: 'because', owned: [], sharedExclusions: [] }
    retirements = {
      schema: 1,
      retired: [
        { ...record, pageId: '1:1' },
        { ...record, pageId: '2:2', componentId: '2:9' }
      ]
    }
  })

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

  describe('when filtering a list', () => {
    let items: { pageId: string; componentId?: string }[]

    beforeEach(() => {
      items = [{ pageId: '3:3' }, { pageId: '1:1', componentId: '1:7' }]
    })

    describe('and nothing is retired', () => {
      it('should keep every item', () => {
        expect(filterRetired(emptyRetirements(), items)).toEqual({ kept: items, retired: [] })
      })
    })

    describe('and the list mixes retired and kept items', () => {
      it('should split retired from kept', () => {
        const { kept, retired } = filterRetired(retirements, items)

        expect(kept).toEqual([{ pageId: '3:3' }])
        expect(retired).toEqual([{ pageId: '1:1', componentId: '1:7' }])
      })
    })
  })

  describe('when serializing equivalent ledgers', () => {
    let data: Retirements
    let before: Retirements

    beforeEach(() => {
      data = structuredClone(retirements)
      data.retired.reverse()
      data.retired[0]!.owned = ['z.tsx', 'a.json#component:2:9']
      data.retired[0]!.sharedExclusions = ['z.svg', 'a.svg']
      before = structuredClone(data)
    })

    it('should serialize them identically without mutating the input', () => {
      const canonical = JSON.parse(serializeRetirements(data)) as Retirements
      expect(canonical.retired.map((entry) => entry.pageId)).toEqual(['1:1', '2:2'])
      expect(canonical.retired[1]!.owned).toEqual(['a.json#component:2:9', 'z.tsx'])
      expect(canonical.retired[1]!.sharedExclusions).toEqual(['a.svg', 'z.svg'])
      expect(serializeRetirements(canonical)).toBe(serializeRetirements(data))
      expect(data).toEqual(before)
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

    describe('and the file is missing', () => {
      it('should return an empty record', async () => {
        expect((await loadRetirements(root)).retired).toEqual([])
      })
    })

    describe('and the file is valid', () => {
      beforeEach(async () => {
        await writeLedger(root, JSON.stringify(retirements))
      })

      it('should parse it', async () => {
        expect((await loadRetirements(root)).retired).toHaveLength(2)
      })
    })

    it.each(['ledger', 'record'])('should reject unknown fields in the %s', async (scope) => {
      const data = structuredClone(retirements)
      Object.assign(scope === 'ledger' ? data : data.retired[0]!, { unexpected: true })
      await writeLedger(root, JSON.stringify(data))
      await expect(loadRetirements(root)).rejects.toThrow('Unrecognized key')
    })

    describe('and the file is malformed', () => {
      beforeEach(async () => {
        await writeLedger(root, '{"schema":2}')
      })

      it('should reject it instead of treating it as empty', async () => {
        await expect(loadRetirements(root)).rejects.toThrow('Invalid input: expected 1')
      })
    })

    describe('and only the cache copy under figma/figma-snapshot exists', () => {
      beforeEach(async () => {
        await mkdir(path.join(root, 'figma/figma-snapshot'), { recursive: true })
        await writeFile(
          path.join(root, 'figma/figma-snapshot/retirements.json'),
          JSON.stringify(retirements)
        )
      })

      it('should point at the committed ledger path', () => {
        expect(RETIREMENTS_PATH).toBe('figma/ledger/retirements.json')
      })

      it('should read the committed ledger, never the cache copy', async () => {
        expect((await loadRetirements(root)).retired).toEqual([])
      })
    })

    // The ledger is committed: a field holds a node id, plain text or a repository path, never a link or key.
    it.each([
      [
        'a malformed entry',
        { pageId: '1:1' },
        'Invalid input: expected string, received undefined'
      ],
      [
        'a Figma URL in the reason',
        { reason: 'see https://www.figma' + '.com/design/abc/x' },
        'must be plain text with no link'
      ],
      [
        'a link in the name',
        { name: 'figma' + '.com/file/abc' },
        'must be plain text with no link'
      ],
      ['markup in the reason', { reason: '<img src=x>' }, 'must be plain text with no link'],
      ['a bidi override in the name', { name: 'Nav\u202EBar' }, 'must be plain text with no link'],
      [
        'a line separator in the reason',
        { reason: 'one\u2028two' },
        'must be plain text with no link'
      ],
      [
        'a javascript link in the reason',
        { reason: 'javascript:alert(1)' },
        'must be plain text with no link'
      ],
      [
        'a node id that is not one',
        { pageId: 'https://example.com/1:1' },
        'must be a Figma node id'
      ],
      ['a component id with a path', { componentId: '1/2' }, 'must be a Figma node id'],
      [
        'an owned path that leaves the repository',
        { owned: ['figma/../../secret.json'] },
        'must not leave the repository'
      ],
      ['an absolute owned path', { owned: ['/etc/passwd'] }, 'must be a repository-relative path'],
      [
        'an owned link',
        { sharedExclusions: ['https://www.figma' + '.com/file/abc/x'] },
        'must not hold a link'
      ]
    ])('should reject %s', async (_, patch, message) => {
      const data = {
        schema: 1,
        retired: [
          'pageId' in patch && Object.keys(patch).length === 1 && patch.pageId === '1:1'
            ? patch
            : { ...retirements.retired[0]!, ...patch }
        ]
      }
      await writeLedger(root, JSON.stringify(data))
      await expect(loadRetirements(root)).rejects.toThrow(message)
      expect(() => serializeRetirements(data as Retirements)).toThrow(message)
    })
  })
})
