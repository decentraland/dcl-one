import { describe, expect, it } from 'vitest'
import { formatRarityLabel, formatSupply, getRarityMaxSupply, getRarityMediaBackground, isRarity } from './rarities'

describe('rarities', () => {
  it('recognises the eight rarities and nothing else', () => {
    expect(isRarity('legendary')).toBe(true)
    expect(isRarity('weird')).toBe(false)
    expect(isRarity(null)).toBe(false)
    expect(getRarityMaxSupply('epic')).toBe(1000)
    expect(getRarityMaxSupply('weird')).toBeUndefined()
  })

  it('formats supplies compactly from a thousand up', () => {
    expect(formatSupply(1)).toBe('1')
    expect(formatSupply(100)).toBe('100')
    expect(formatSupply(1000)).toBe('1K')
    expect(formatSupply(5000)).toBe('5K')
    expect(formatSupply(100000)).toBe('100K')
  })

  it('appends the supply to a label only for known rarities', () => {
    expect(formatRarityLabel('Legendary', 'legendary')).toBe('Legendary (100)')
    expect(formatRarityLabel('Epic', 'epic')).toBe('Epic (1K)')
    expect(formatRarityLabel('weird', 'weird')).toBe('weird')
  })

  it('tints the media with the rarity color and leaves unknown rarities neutral', () => {
    expect(getRarityMediaBackground('legendary')).toMatch(/^radial-gradient\(.*rgba\(162, 75, 243, 0\.62\) 100%\)$/)
    expect(getRarityMediaBackground(undefined)).toBeUndefined()
  })
})
