import { describe, expect, it } from 'vitest'
import { CREDIT_PACKS, MAX_PACK_QUANTITY, formatUsd, recommendPack, selectionTotals } from './creditPacks'

describe('recommendPack', () => {
  it('preselects the cheapest single pack that covers the shortfall', () => {
    expect(recommendPack(CREDIT_PACKS, 240)).toEqual({ packId: 'pack_25', quantity: 1 })
    expect(recommendPack(CREDIT_PACKS, 490)).toEqual({ packId: 'pack_50', quantity: 1 })
    expect(recommendPack(CREDIT_PACKS, 40)).toEqual({ packId: 'pack_5', quantity: 1 })
  })

  it('buys as many of the largest pack as it takes when no single pack covers it', () => {
    expect(recommendPack(CREDIT_PACKS, 1000)).toEqual({ packId: 'pack_50', quantity: 2 })
    expect(recommendPack(CREDIT_PACKS, 600)).toEqual({ packId: 'pack_50', quantity: 2 })
    expect(recommendPack(CREDIT_PACKS, 1081)).toEqual({ packId: 'pack_50', quantity: 3 })
  })

  it('never asks for more than one checkout can buy', () => {
    expect(recommendPack(CREDIT_PACKS, 100_000)).toEqual({ packId: 'pack_50', quantity: MAX_PACK_QUANTITY })
  })

  it('offers the smallest pack when nothing is missing, and nothing without a catalogue', () => {
    expect(recommendPack(CREDIT_PACKS, 0)).toEqual({ packId: 'pack_5', quantity: 1 })
    expect(recommendPack([], 300)).toBeNull()
  })
})

describe('selectionTotals', () => {
  it('multiplies credits and dollars, keeping the price to cents', () => {
    expect(selectionTotals(CREDIT_PACKS, { packId: 'pack_50', quantity: 2 })).toEqual({ credits: 1080, usd: 119.98 })
    expect(selectionTotals(CREDIT_PACKS, { packId: 'pack_5', quantity: 3 })).toEqual({ credits: 120, usd: 17.97 })
  })

  it('is empty without a selection or for an unknown pack', () => {
    expect(selectionTotals(CREDIT_PACKS, null)).toEqual({ credits: 0, usd: 0 })
    expect(selectionTotals(CREDIT_PACKS, { packId: 'pack_999', quantity: 1 })).toEqual({ credits: 0, usd: 0 })
  })
})

describe('formatUsd', () => {
  it('renders dollars with two decimals and thousands separators', () => {
    expect(formatUsd(5.99)).toBe('$5.99')
    expect(formatUsd(1199.9)).toBe('$1,199.90')
  })
})
