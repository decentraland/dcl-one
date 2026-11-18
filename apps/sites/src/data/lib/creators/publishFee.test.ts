import { describe, expect, it } from 'vitest'
import { type BlockchainRarity } from './rarities'
import {
  creditsForUsdWei,
  formatCredits,
  formatMana,
  getPublicationFee,
  usdCentsToCredits,
  weiToUsdCents
} from './publishFee'

const ETHER = 10n ** 18n

// $5 per item at a 100 MANA/$ oracle rate → 50 credits / 500 MANA per item.
const RARITIES: BlockchainRarity[] = [
  {
    id: 'common',
    name: 'common',
    price: '5000000000000000000',
    maxSupply: '100000',
    prices: { USD: '5000000000000000000', MANA: '500000000000000000000' }
  },
  {
    id: 'epic',
    name: 'epic',
    price: '5000000000000000000',
    maxSupply: '1000',
    prices: { USD: '5000000000000000000', MANA: '500000000000000000000' }
  }
]

describe('weiToUsdCents', () => {
  it('converts USD wei to cents, rounding any fraction of a cent up', () => {
    expect(weiToUsdCents(5n * ETHER)).toBe(500)
    expect(weiToUsdCents(10n ** 16n)).toBe(1)
    expect(weiToUsdCents(10n ** 16n + 1n)).toBe(2)
    expect(weiToUsdCents(0n)).toBe(0)
  })
})

describe('credits conversion', () => {
  it('charges whole credits, rounding up (1 credit = 10 cents)', () => {
    expect(usdCentsToCredits(500)).toBe(50)
    expect(usdCentsToCredits(501)).toBe(51)
    expect(creditsForUsdWei(5n * ETHER)).toBe(50)
  })
})

describe('getPublicationFee', () => {
  it('prices every item at the first rarity and totals credits, USD and MANA', () => {
    const fee = getPublicationFee(RARITIES, 6)!
    expect(fee.itemCount).toBe(6)
    expect(fee.perItem).toEqual({ manaWei: 500n * ETHER, usdWei: 5n * ETHER, credits: 50 })
    expect(fee.total).toEqual({ manaWei: 3000n * ETHER, usdWei: 30n * ETHER, credits: 300 })
    expect(fee.manaPerCredit).toBe(10n * ETHER)
  })

  it('is null until the rarities carry prices', () => {
    expect(getPublicationFee([], 3)).toBeNull()
    expect(getPublicationFee([{ id: 'epic', name: 'epic', price: '1', maxSupply: '1000' }], 3)).toBeNull()
  })

  it('is free with a zero-priced rarity and never divides by zero', () => {
    const free = RARITIES.map(r => ({ ...r, prices: { USD: '0', MANA: '0' } }))
    const fee = getPublicationFee(free, 2)!
    expect(fee.total.credits).toBe(0)
    expect(fee.manaPerCredit).toBe(0n)
  })
})

describe('formatting', () => {
  it('formats MANA compactly and credits as whole numbers', () => {
    expect(formatMana(3000n * ETHER)).toBe('3,000')
    expect(formatMana(12n * ETHER + ETHER / 2n)).toBe('12.5')
    expect(formatMana(ETHER / 3n)).toBe('0.33')
    expect(formatCredits(1234)).toBe('1,234')
  })
})
