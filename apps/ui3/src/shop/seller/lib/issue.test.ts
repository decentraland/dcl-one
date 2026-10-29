// from: shop@94584a5c:app/src/lib/issue.spec.ts (ported with its module)
import { describe, it, expect, vi } from 'vitest'

// decentraland-transactions ships an ESM build that Node can't directory-import under vitest; mock it
// (mirrors transfer-item.spec / buy.spec) so importing ~/lib/issue and its deps loads cleanly. The pure
// helpers under test never touch it.
vi.mock('decentraland-transactions', () => ({
  ContractName: { ERC721CollectionV2: 'ERC721CollectionV2' },
  getContractName: () => 'DecentralandMarketplacePolygon',
  getContract: () => ({ address: '0xcollection', abi: [], name: 'Decentraland Collection', version: '2' }),
  sendMetaTransaction: vi.fn(() => Promise.resolve('0xrelayhash')),
  MetaTransactionError: class extends Error {},
  ErrorCode: { USER_DENIED: 'USER_DENIED' }
}))
vi.mock('~/config', () => ({ config: { chainId: 80002, rpcUrl: 'http://localhost' } }))

import { buildIssueArgs, isIssueValid, isValidIssueAddress, totalToIssue, type IssueEntry } from './issue'

const A = '0x1111111111111111111111111111111111111111'
const B = '0x2222222222222222222222222222222222222222'

describe('lib/issue', () => {
  describe('isValidIssueAddress', () => {
    it('should accept a 40-hex 0x address (trimming whitespace)', () => {
      expect(isValidIssueAddress(A)).toBe(true)
      expect(isValidIssueAddress(`  ${A}  `)).toBe(true)
    })
    it('should reject non-addresses', () => {
      expect(isValidIssueAddress('')).toBe(false)
      expect(isValidIssueAddress('0x123')).toBe(false)
      expect(isValidIssueAddress('nope')).toBe(false)
    })
  })

  describe('totalToIssue', () => {
    it('should sum floored, non-negative amounts across rows', () => {
      const rows: IssueEntry[] = [
        { address: A, amount: 3 },
        { address: B, amount: 2 }
      ]
      expect(totalToIssue(rows)).toBe(5)
    })
    it('should ignore blank/negative/NaN amounts', () => {
      const rows: IssueEntry[] = [
        { address: A, amount: Number('') },
        { address: B, amount: -4 },
        { address: A, amount: 2 }
      ]
      expect(totalToIssue(rows)).toBe(2)
    })
  })

  describe('buildIssueArgs', () => {
    it('should repeat the address + itemId once per copy, index-aligned', () => {
      const rows: IssueEntry[] = [
        { address: A, amount: 2 },
        { address: B, amount: 1 }
      ]
      const { beneficiaries, itemIds } = buildIssueArgs(rows, '7')
      expect(beneficiaries).toEqual([A, A, B])
      expect(itemIds).toEqual(['7', '7', '7'])
      expect(beneficiaries).toHaveLength(itemIds.length)
    })

    it('should trim addresses and skip empty / zero-amount rows', () => {
      const rows: IssueEntry[] = [
        { address: `  ${A}  `, amount: 1 },
        { address: '', amount: 5 },
        { address: B, amount: 0 }
      ]
      const { beneficiaries, itemIds } = buildIssueArgs(rows, '0')
      expect(beneficiaries).toEqual([A])
      expect(itemIds).toEqual(['0'])
    })

    it('should floor fractional amounts', () => {
      const { beneficiaries } = buildIssueArgs([{ address: A, amount: 2.9 }], '1')
      expect(beneficiaries).toEqual([A, A])
    })
  })

  describe('isIssueValid (supply cap + row validation)', () => {
    it('should accept rows whose total is within the available supply', () => {
      expect(isIssueValid([{ address: A, amount: 3 }], 5)).toBe(true)
      expect(isIssueValid([{ address: A, amount: 5 }], 5)).toBe(true) // exactly the cap
    })
    it('should reject when the total exceeds the available supply', () => {
      expect(isIssueValid([{ address: A, amount: 6 }], 5)).toBe(false)
      expect(
        isIssueValid(
          [
            { address: A, amount: 3 },
            { address: B, amount: 3 }
          ],
          5
        )
      ).toBe(false)
    })
    it('should reject an invalid address in any non-empty row', () => {
      expect(isIssueValid([{ address: '0xbad', amount: 1 }], 5)).toBe(false)
    })
    it('should reject an amount below 1 or non-integer', () => {
      expect(isIssueValid([{ address: A, amount: 0 }], 5)).toBe(false)
      expect(isIssueValid([{ address: A, amount: 1.5 }], 5)).toBe(false)
    })
    it('should reject when there is nothing to issue', () => {
      expect(isIssueValid([{ address: '', amount: 0 }], 5)).toBe(false)
      expect(isIssueValid([], 5)).toBe(false)
    })
    it('should ignore a trailing blank row while validating the filled ones', () => {
      const rows: IssueEntry[] = [
        { address: A, amount: 2 },
        { address: '', amount: 0 }
      ]
      expect(isIssueValid(rows, 5)).toBe(true)
    })
  })
})
