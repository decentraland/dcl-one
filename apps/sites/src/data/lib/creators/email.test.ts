import { describe, expect, it } from 'vitest'
import { isValidEmail } from './email'

describe('isValidEmail', () => {
  it('accepts ordinary addresses, ignoring surrounding spaces', () => {
    expect(isValidEmail('jane.doe@example.com')).toBe(true)
    expect(isValidEmail('jane+hats@mail.example.co')).toBe(true)
    expect(isValidEmail('  jane.doe@example.com ')).toBe(true)
    expect(isValidEmail('jane@example.xn--p1ai')).toBe(true)
    expect(isValidEmail('-jane_doe!@example.com')).toBe(true)
  })

  it('rejects addresses the creator would not get mail at', () => {
    expect(isValidEmail('')).toBe(false)
    expect(isValidEmail('jane.doe')).toBe(false)
    expect(isValidEmail('jane.doe@example')).toBe(false)
    expect(isValidEmail('jane doe@example.com')).toBe(false)
    expect(isValidEmail('jane.doe@example.c')).toBe(false)
    expect(isValidEmail('.jane@example.com')).toBe(false)
    expect(isValidEmail('jane.@example.com')).toBe(false)
    expect(isValidEmail('jane..doe@example.com')).toBe(false)
    expect(isValidEmail('...@example.com')).toBe(false)
    expect(isValidEmail('jane.doe@example.42')).toBe(false)
    expect(isValidEmail(`${'a'.repeat(250)}@example.com`)).toBe(false)
  })
})
