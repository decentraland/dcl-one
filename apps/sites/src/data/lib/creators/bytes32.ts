import { isHex, pad, size, type Hex } from 'viem'

/** A bytes32 word from hex of up to 32 bytes, left-padded; anything else throws instead of encoding garbage. */
export function toBytes32(value: string): Hex {
  if (!isHex(value, { strict: true }) || size(value) > 32) {
    throw new Error(`Expected hex of up to 32 bytes, got ${value}`)
  }
  return pad(value, { size: 32 })
}
