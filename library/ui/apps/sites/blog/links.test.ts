import { describe, expect, it } from 'vitest'
import { isSafeHyperlinkUri } from './links'

// The one link allowlist, used by the post renderer and the post markdown for agents.
describe('isSafeHyperlinkUri', () => {
  it.each(['https://example.org', 'http://example.org', 'mailto:a@example.org', '/blog/x', '#top'])(
    'allows %j',
    (uri) => {
      expect(isSafeHyperlinkUri(uri)).toBe(true)
    }
  )

  it.each([
    'javascript:alert(1)',
    ' javascript:alert(1)',
    'data:text/html,x',
    'vbscript:x',
    'decentraland://open',
    'decentraland.org/download',
    ''
  ])('refuses %j', (uri) => {
    expect(isSafeHyperlinkUri(uri)).toBe(false)
  })

  it('refuses a value that is not a string', () => {
    expect(isSafeHyperlinkUri(undefined)).toBe(false)
    expect(isSafeHyperlinkUri(42)).toBe(false)
  })
})
