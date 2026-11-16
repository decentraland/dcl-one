import { describe, it, expect } from 'vitest'
import { userNameColor } from './nameColor'

describe('userNameColor', () => {
  // The engine's `name_color` (`crates/comms/src/profile/name_color.rs`) checks the same vectors.
  it.each([
    ['Ada', '0x00000000000000000000000000000000000012ab', true, '#4087ff'],
    ['Ada', '0x00000000000000000000000000000000000012ab', false, '#4078ff'],
    ['Grace Hopper!', '0xAbCdEf0000000000000000000000000000009F3c', false, '#40dfff'],
    ['名前😀', '0x0000000000000000000000000000000000000001', true, '#4f40ff'],
    ['', '0x0000000000000000000000000000000000000001', true, '#ffffff'],
    ['x#1234', '0x00000000000000000000000000000000deadbeef', false, '#40daff'],
    // a combining vowel sign is neither a letter nor a number
    ['राम', '0x0000000000000000000000000000000000000001', true, '#ff4092']
  ])('colours %j (%s, claimed %s) %s', (name, address, claimed, expected) => {
    expect(userNameColor(name, address, claimed)).toBe(expected)
  })
})
