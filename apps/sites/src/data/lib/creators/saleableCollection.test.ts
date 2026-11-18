import { describe, expect, it } from 'vitest'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { type Collection } from './collections'
import { ItemType, type Item } from './items'
import { type ItemListing } from './listings'
import { discountedPriceCredits, manaToCredits, toSaleableCollection, type SaleItem } from './saleableCollection'

const CHAIN_ID = 80002
const V3 = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID).address.toLowerCase()
const MANA = 10n ** 18n
const collection = { id: 'c1', name: 'Spooky', contractAddress: '0xcc', itemCount: 4 } as Collection

const item = (tokenId: string, totalSupply = 0): Item =>
  ({
    id: `i${tokenId}`,
    name: `Item ${tokenId}`,
    tokenId,
    rarity: 'legendary',
    totalSupply,
    thumbnail: 'thumbnail.png',
    contents: { 'thumbnail.png': `Qm${tokenId}` },
    type: ItemType.WEARABLE
  }) as unknown as Item

const listings = new Map<string, ItemListing>([
  ['0', { itemId: '0', tradeId: 't0', tradeContract: V3, currency: 'credits', credits: 50 }],
  ['1', { itemId: '1', tradeId: 't1', tradeContract: V3, currency: 'mana', manaWei: 20n * MANA }],
  ['2', { itemId: '2', currency: 'mana', manaWei: 5n * MANA }]
])

describe('when reading a collection the way the discount modal does', () => {
  it('should split its items into discounted, classic and unlisted, leaving sold-out ones out', () => {
    const sale = toSaleableCollection(
      collection,
      [item('0'), item('1'), item('2'), item('3'), item('4', 100)],
      listings,
      {
        chainId: CHAIN_ID,
        manaListings: false
      }
    )
    expect(sale.items.map(i => [i.key, i.state])).toEqual([
      ['0xcc-0', 'discounted'],
      ['0xcc-1', 'classic'],
      ['0xcc-2', 'classic'],
      ['0xcc-3', 'unlisted']
    ])
    expect(sale.listedCount).toBe(1)
    expect(sale.items[0]).toMatchObject({ priceCredits: 50, remainingSupply: 100 })
    expect(sale.items[0].thumbnail).toContain('Qm0')
  })

  it('should price a discounted MANA listing in credits at the live rate, and leave it unpriced without one', () => {
    const on = { chainId: CHAIN_ID, manaListings: true }
    const withRate = toSaleableCollection(collection, [item('1')], listings, { ...on, manaRate: 9n * 10n ** 16n })
    expect(withRate.items[0]).toMatchObject({ state: 'discounted', mana: true, priceCredits: 18, manaWei: 20n * MANA })
    expect(toSaleableCollection(collection, [item('1')], listings, on).items[0].priceCredits).toBeNull()
  })
})

describe('when converting MANA to credits', () => {
  it('should round up to a whole credit', () => {
    expect(manaToCredits(10n * MANA, 10n ** 17n)).toBe(10)
    expect(manaToCredits(10n * MANA + 5n * 10n ** 16n, 10n ** 17n)).toBe(11)
    // Never below one credit, the Shop's floor for any price.
    expect(manaToCredits(1n, 10n ** 17n)).toBe(1)
  })
})

describe('when quoting a discounted item in credits', () => {
  const rate = 11n * 10n ** 16n // $0.11 per MANA

  // The contract discounts the MANA amount, and the Shop converts what it sells for: 0.7 MANA is $0.077, 1 credit.
  // Discounting the rounded 2-credit list price instead would quote 2.
  it('should discount a MANA price in MANA before converting it', () => {
    const item: SaleItem = {
      key: 'k',
      name: 'Hat',
      thumbnail: '',
      priceCredits: manaToCredits(MANA, rate),
      manaWei: MANA,
      state: 'discounted',
      mana: true,
      remainingSupply: 1
    }
    expect(item.priceCredits).toBe(2)
    expect(discountedPriceCredits(item, 30, rate)).toBe(1)
  })

  it('should discount a credits price the way checkout rounds it', () => {
    const item: SaleItem = {
      key: 'k',
      name: 'Hat',
      thumbnail: '',
      priceCredits: 5,
      state: 'discounted',
      remainingSupply: 1
    }
    expect(discountedPriceCredits(item, 30, undefined)).toBe(4)
  })
})
