import { describe, expect, it } from 'vitest'
import { ContractName, getContract, getCouponManager, type MarketplaceName } from '~/lib/chain/contracts'
import { discountedCredits, discountedManaWei, runningDiscountPpm, takesDiscount } from './collectionDiscount'
import { type CollectionDiscount } from './coupons'
import { type ItemListing } from './listings'

const CHAIN_ID = 80002
const V3 = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID).address.toLowerCase()
const V2 = getContract(ContractName.OffChainMarketplaceV2, CHAIN_ID).address.toLowerCase()
const MANA = 10n ** 18n

const credits = (itemId: string, amount: number, tradeContract = V3): ItemListing => ({
  itemId,
  tradeId: `t-${itemId}`,
  tradeContract,
  currency: 'credits',
  credits: amount
})
const manaTrade = (itemId: string, manaWei: bigint, tradeContract = V3): ItemListing => ({
  itemId,
  tradeId: `t-${itemId}`,
  tradeContract,
  currency: 'mana',
  manaWei
})
// A collection-store price: bought through CollectionStore, which takes no coupon.
const storePrice = (itemId: string): ItemListing => ({ itemId, currency: 'mana', manaWei: 10n * MANA })

const off = { chainId: CHAIN_ID, manaListings: false }
const on = { chainId: CHAIN_ID, manaListings: true }

describe('when deciding whether a new discount re-prices a listing', () => {
  it('should discount a credits listing on the current marketplace', () => {
    expect(takesDiscount(credits('0', 50), off)).toBe(true)
  })

  it('should discount a MANA listing only while the Shop discounts MANA listings', () => {
    expect(takesDiscount(manaTrade('0', 10n * MANA), on)).toBe(true)
    expect(takesDiscount(manaTrade('0', 10n * MANA), off)).toBe(false)
  })

  // That marketplace redeems through another manager, so the coupon never reaches its checkout.
  it('should leave a listing on an older marketplace alone', () => {
    expect(takesDiscount(credits('0', 50, V2), on)).toBe(false)
    expect(takesDiscount(manaTrade('0', 10n * MANA, V2), on)).toBe(false)
  })

  it('should never discount a collection-store price or a giveaway', () => {
    expect(takesDiscount(storePrice('0'), on)).toBe(false)
    expect(takesDiscount(manaTrade('0', 0n), on)).toBe(false)
  })
})

describe('when pricing the example line', () => {
  it('should round a credits price up to a whole credit, never below one', () => {
    expect(discountedCredits(100, 30)).toBe(70)
    expect(discountedCredits(5, 30)).toBe(4)
    expect(discountedCredits(1, 70)).toBe(1)
  })

  it('should price MANA the way the contract does: price minus the floored cut', () => {
    expect(discountedManaWei(100n * MANA, 30)).toBe(70n * MANA)
    expect(discountedManaWei(7n, 30)).toBe(5n)
  })
})

describe('when picking the running discount a buyer gets on a listing', () => {
  const NOW = Date.UTC(2026, 9, 19)
  const DAY = 86_400_000
  const managerOf = (name: MarketplaceName) => getCouponManager(name, CHAIN_ID).address.toLowerCase()
  const discount = (ppm: number, overrides: Partial<CollectionDiscount> = {}) =>
    ({
      id: `d-${ppm}`,
      discountType: 1,
      discount: ppm,
      couponManager: managerOf(ContractName.OffChainMarketplaceV3),
      status: 'active',
      checks: { effective: NOW - DAY, expiration: NOW + DAY },
      ...overrides
    }) as unknown as CollectionDiscount

  it('should take the deepest one that has started', () => {
    const running = [
      discount(100_000),
      discount(300_000),
      discount(500_000, { checks: { effective: NOW + DAY, expiration: NOW + 2 * DAY } as never })
    ]
    expect(runningDiscountPpm(credits('0', 50), running, off, NOW)).toBe(300_000)
  })

  // Each marketplace redeems only its own manager's coupons, whatever a new discount would be signed against.
  it("should only count discounts signed with the manager the listing's marketplace trusts", () => {
    const onV2 = discount(400_000, { couponManager: managerOf(ContractName.OffChainMarketplaceV2) })
    expect(runningDiscountPpm(credits('0', 50), [onV2], off, NOW)).toBeUndefined()
    expect(runningDiscountPpm(credits('0', 50, V2), [onV2], off, NOW)).toBe(400_000)
  })

  it('should leave MANA listings alone while the Shop does not discount them', () => {
    expect(runningDiscountPpm(manaTrade('0', 10n * MANA), [discount(300_000)], off, NOW)).toBeUndefined()
    expect(runningDiscountPpm(manaTrade('0', 10n * MANA), [discount(300_000)], on, NOW)).toBe(300_000)
  })

  it('should leave a collection-store price alone', () => {
    expect(runningDiscountPpm(storePrice('0'), [discount(300_000)], on, NOW)).toBeUndefined()
  })
})
