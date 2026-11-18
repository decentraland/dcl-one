import { describe, it, expect } from 'vitest'
import { shortenAddress } from './address'

describe('shortenAddress', () => {
  it('keeps the 0x prefix with the first and last four characters', () => {
    expect(shortenAddress('0x1d9fb685c257e74f869ba302e260c0b68f5ebb37')).toBe('0x1d9f…bb37')
  })
})
