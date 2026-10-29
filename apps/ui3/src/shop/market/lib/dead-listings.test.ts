// from: shop@94584a5c:app/src/lib/dead-listings.spec.ts (ported with its module)
import { describe, it, expect, beforeEach } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { liveTradeId, markListingCancelled } from './dead-listings'

let qc: QueryClient

beforeEach(() => {
  qc = new QueryClient()
})

describe('liveTradeId', () => {
  describe('when nothing has been taken down', () => {
    it('should report the trade id unchanged', () => {
      expect(liveTradeId(qc, 'trade-1')).toBe('trade-1')
    })
  })

  describe('when the trade has been taken down', () => {
    it('should report no live trade', () => {
      markListingCancelled(qc, 'trade-1')

      expect(liveTradeId(qc, 'trade-1')).toBeUndefined()
    })

    // The whole point of remembering exact ids rather than a "just cancelled" window: a re-list signs a NEW
    // trade, and other sellers' listings are unrelated. Suppressing those would hide real liquidity.
    it('should leave every other trade buyable', () => {
      markListingCancelled(qc, 'trade-1')

      expect(liveTradeId(qc, 'trade-2')).toBe('trade-2')
    })
  })

  describe('when several listings have been taken down', () => {
    it('should report all of them as gone', () => {
      markListingCancelled(qc, 'trade-1')
      markListingCancelled(qc, 'trade-2')

      expect(liveTradeId(qc, 'trade-1')).toBeUndefined()
      expect(liveTradeId(qc, 'trade-2')).toBeUndefined()
    })
  })

  describe('when the same listing is marked twice', () => {
    it('should not duplicate the entry', () => {
      markListingCancelled(qc, 'trade-1')
      markListingCancelled(qc, 'trade-1')

      expect(qc.getQueryData<string[]>(['cancelled-listings'])).toEqual(['trade-1'])
    })
  })

  describe('when there is no trade id at all', () => {
    it('should report no live trade', () => {
      expect(liveTradeId(qc, undefined)).toBeUndefined()
      expect(liveTradeId(qc, null)).toBeUndefined()
      expect(liveTradeId(qc, '')).toBeUndefined()
    })
  })
})

describe('markListingCancelled', () => {
  describe('when the trade id is empty', () => {
    it('should record nothing', () => {
      markListingCancelled(qc, '')

      expect(qc.getQueryData(['cancelled-listings'])).toBeUndefined()
    })
  })
})
