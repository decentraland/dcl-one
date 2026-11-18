import { describe, expect, it } from 'vitest'
import { isUuid, parseUuidParam } from './ids'

const ID = '3f2b8c1e-9a4d-4e7f-8b21-0c6d5e4f3a2b'

describe('URL ids', () => {
  it('accepts a UUID in either case', () => {
    expect(isUuid(ID)).toBe(true)
    expect(isUuid(ID.toUpperCase())).toBe(true)
    expect(parseUuidParam(ID)).toBe(ID)
  })

  it('refuses anything that could rewrite a request path or is simply not an id', () => {
    for (const value of ['../items/abc', `${ID}/items`, ' ' + ID, ID.slice(1), 'null', '', null, undefined]) {
      expect(isUuid(value)).toBe(false)
      expect(parseUuidParam(value)).toBeUndefined()
    }
  })
})
