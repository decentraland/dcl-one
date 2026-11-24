import { describe, expect, it } from 'vitest'
import { blogHref } from './blogLinks'

describe('blogHref', () => {
  describe('when the uri is an #anchor', () => {
    it('should keep it internal', () => {
      expect(blogHref('#faq')).toEqual({ kind: 'internal', href: '#faq' })
    })
  })

  describe('when the uri is a site path', () => {
    it('should keep it internal', () => {
      expect(blogHref('/blog/platform/a')).toEqual({ kind: 'internal', href: '/blog/platform/a' })
    })
  })

  describe('when the uri uses http, https or mailto', () => {
    it.each(['https://example.org/a', 'http://example.org', 'mailto:hi@example.org'])(
      'should open %s as external',
      (uri) => {
        expect(blogHref(uri)).toEqual({ kind: 'external', href: uri })
      }
    )
  })

  describe('when the uri is an unsafe scheme, protocol-relative or junk', () => {
    it.each([
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
    ])('should drop %j', (uri) => {
      expect(blogHref(uri)).toBeNull()
    })
  })
})
