import { describe, expect, it } from 'vitest'
import { blogHref } from './blogLinks'

describe('blogHref', () => {
  it('keeps #anchors and site paths internal', () => {
    expect(blogHref('#faq')).toEqual({ kind: 'internal', href: '#faq' })
    expect(blogHref('/blog/platform/a')).toEqual({ kind: 'internal', href: '/blog/platform/a' })
  })

  it('opens http, https and mailto as external', () => {
    for (const uri of ['https://example.org/a', 'http://example.org', 'mailto:hi@example.org'])
      expect(blogHref(uri)).toEqual({ kind: 'external', href: uri })
  })

  it('drops unsafe schemes, protocol-relative URLs and junk', () => {
    for (const uri of [
      'javascript:alert(1)',
      ' JavaScript:alert(1)',
      'data:text/html,x',
      'vbscript:x',
      '//evil.example/x',
      '/\\evil.example',
      'not a url',
      '',
      '   ',
      42,
      undefined
    ])
      expect(blogHref(uri)).toBeNull()
  })
})
