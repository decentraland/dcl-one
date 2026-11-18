import { describe, expect, it, vi } from 'vitest'
import { getPublicationFee } from './publishFee'
import { type BlockchainRarity } from './rarities'
import {
  PublicationFeeMismatchError,
  isWithinFeeTolerance,
  verifyPublicationFee,
  withFeeTolerance
} from './feeVerification'

const ETHER = 10n ** 18n
const CHAIN_ID = 80002
// $5 per item, 500 MANA at the quoted oracle rate.
const RARITIES: BlockchainRarity[] = [
  {
    id: 'common',
    name: 'common',
    price: '5',
    maxSupply: '100000',
    prices: { USD: '5', MANA: (500n * ETHER).toString() }
  }
]
const items = [{ rarity: 'common' }, { rarity: 'common' }, { rarity: 'epic' }]
const fee = getPublicationFee(RARITIES, items.length)!

describe('verifyPublicationFee', () => {
  it('accepts a quote the contract would charge, allowing for oracle drift', async () => {
    const read = vi.fn().mockResolvedValue(500n * ETHER)
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, read)).resolves.toBe(1500n * ETHER)
    expect(read).toHaveBeenCalledTimes(2)
    expect(read).toHaveBeenCalledWith('common', CHAIN_ID)
    expect(read).toHaveBeenCalledWith('epic', CHAIN_ID)

    // Within tolerance, it answers what the contract charges, not the quote: that is what the approval must cover.
    const drifted = vi.fn().mockResolvedValue(515n * ETHER)
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, drifted)).resolves.toBe(1545n * ETHER)
  })

  it('weighs each rarity by its own on-chain price and item count', async () => {
    // 2 common × 300 + 1 epic × 900 = 1500 MANA: matches the quote although neither price is the quoted 500.
    const perRarity = vi.fn((rarity: string) => Promise.resolve((rarity === 'epic' ? 900n : 300n) * ETHER))
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, perRarity)).resolves.toBe(1500n * ETHER)
    // 2 common × 300 + 1 epic × 300 = 900 MANA: the same per-rarity read, priced flat, no longer matches.
    const flat = vi.fn().mockResolvedValue(300n * ETHER)
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, flat)).rejects.toMatchObject({ onChainWei: 900n * ETHER })
  })

  it('refuses a quote that is not what the contract charges', async () => {
    const cheaper = vi.fn().mockResolvedValue(700n * ETHER)
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, cheaper)).rejects.toBeInstanceOf(
      PublicationFeeMismatchError
    )
    const overQuoted = vi.fn().mockResolvedValue(100n * ETHER)
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, overQuoted)).rejects.toMatchObject({
      quotedWei: 1500n * ETHER,
      onChainWei: 300n * ETHER
    })
    const free = vi.fn().mockResolvedValue(0n)
    await expect(verifyPublicationFee(fee, items, CHAIN_ID, free)).rejects.toBeInstanceOf(PublicationFeeMismatchError)
  })

  it('treats a free fee as matching a free contract', () => {
    expect(isWithinFeeTolerance(0n, 0n)).toBe(true)
    expect(isWithinFeeTolerance(1n, 0n)).toBe(false)
    expect(isWithinFeeTolerance(1050n, 1000n)).toBe(true)
    expect(isWithinFeeTolerance(1051n, 1000n)).toBe(false)
  })
})

describe('withFeeTolerance', () => {
  it('adds the fee tolerance as headroom, rounded up', () => {
    expect(withFeeTolerance(1000n * ETHER)).toBe(1050n * ETHER)
    expect(withFeeTolerance(1n)).toBe(2n)
    expect(withFeeTolerance(0n)).toBe(0n)
  })

  it('covers any on-chain total the verification would have accepted', () => {
    const quoted = 1000n * ETHER
    const onChain = 1050n * ETHER
    expect(isWithinFeeTolerance(quoted, onChain)).toBe(true)
    expect(withFeeTolerance(quoted) >= onChain).toBe(true)
  })
})
