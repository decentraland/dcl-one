import { describe, expect, it } from 'vitest'
import { toBytes32 } from './bytes32'

describe('toBytes32', () => {
  it('should left-pad shorter hex to 32 bytes', () => {
    expect(toBytes32('0x01')).toBe('0x' + '00'.repeat(31) + '01')
    expect(toBytes32('0x')).toBe('0x' + '00'.repeat(32))
  })

  it('should keep a 32-byte value as it is', () => {
    const word = '0x' + 'ab'.repeat(32)
    expect(toBytes32(word)).toBe(word)
  })

  it('should refuse a value that is not hex instead of encoding it', () => {
    expect(() => toBytes32('6f1c2a40-0000-4000-8000-000000000000')).toThrow()
    expect(() => toBytes32('0xzz')).toThrow()
  })

  it('should refuse hex longer than 32 bytes', () => {
    expect(() => toBytes32('0x' + '11'.repeat(33))).toThrow()
  })
})
