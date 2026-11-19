import { describe, expect, it } from 'vitest'
import { cssUrl } from './cssUrl'

describe('cssUrl', () => {
  it('keeps a plain address as it is', () => {
    expect(cssUrl('/content/contents/bafy1')).toBe('url("/content/contents/bafy1")')
    expect(cssUrl('https://example.org/a b.png?x=(1)')).toBe(
      'url("https://example.org/a b.png?x=(1)")'
    )
  })

  it('escapes a quote and a backslash, so the address cannot end the string', () => {
    expect(cssUrl('https://example.org/a.png"); color: red; x: url("')).toBe(
      'url("https://example.org/a.png\\"); color: red; x: url(\\"")'
    )
    expect(cssUrl('a\\b')).toBe('url("a\\\\b")')
  })

  it('removes a line break, which ends a CSS string', () => {
    expect(cssUrl('https://example.org/a\r\n.png')).toBe('url("https://example.org/a.png")')
  })
})
