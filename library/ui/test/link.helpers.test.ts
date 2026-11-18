import { describe, expect, expectTypeOf, it } from 'vitest'
import type { PolymorphicLinkProps } from '../link.helpers'
import {
  assertNavigationEntries,
  linkHref,
  secureRel,
  isPlainLinkClick,
  hasUnsafeURLCharacters
} from '../link.helpers'

describe('when resolving shared native link destinations', () => {
  it.each([undefined, null, false, true, 0, 1, {}, [], { trim: 'invalid' }])(
    'should suppress non-string destination %j without throwing',
    (value) => {
      expect(linkHref(value as never)).toBeUndefined()
    }
  )
  it.each([
    '',
    '  ',
    'javascript:alert(1)',
    'data:text/html,hi',
    'ftp://example.org',
    'https://user:pass@example.org',
    'https://example.org/\\bad',
    'https://example.org/a\nb',
    'https://example.org/a\u0000b'
  ])('should reject invalid or unsafe destination %j', (value) => {
    expect(linkHref(value)).toBeUndefined()
    expect(linkHref(value, true)).toBeUndefined()
  })
  it.each([
    '/path',
    './path',
    '../path',
    '#section',
    '?q=hello',
    '//example.org/path',
    'https://example.org',
    'http://example.org'
  ])('should preserve native safe destination %s', (value) => {
    expect(linkHref(value)).toBe(value)
    expect(linkHref('  ' + value + '  ')).toBe(value)
  })
  it.each(['decentraland://realm/path', 'intent://realm/#Intent;scheme=https;end'])(
    'should require explicit launch ownership for %s',
    (value) => {
      expect(linkHref(value)).toBeUndefined()
      expect(linkHref(value, true)).toBe(value)
    }
  )
})
describe('when hardening shared native browsing contexts', () => {
  it.each([undefined, '', '_self', '_parent', '_top', '_SELF'])(
    'should preserve same-context relations for %j',
    (target) => {
      expect(secureRel(target, 'author')).toBe('author')
    }
  )
  it.each(['_blank', 'named', '_BLANK'])(
    'should harden new browsing context %s without duplicate tokens',
    (target) => {
      expect(secureRel(target, 'author noopener author')).toBe('author noopener noreferrer')
    }
  )
})
describe('when observing native primary link selection', () => {
  const plain = {
    defaultPrevented: false,
    button: 0,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false
  }
  it('should accept an unclaimed primary activation', () => {
    expect(isPlainLinkClick(plain)).toBe(true)
  })
  it.each([
    { button: 1 },
    { button: 2 },
    { defaultPrevented: true },
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { shiftKey: true }
  ])('should preserve native or claimed activation %j', (change) => {
    expect(isPlainLinkClick({ ...plain, ...change })).toBe(false)
  })
})

describe('when shared URL syntax is checked before browser parsing', () => {
  it.each([...Array.from({ length: 32 }, (_, index) => index), 127])(
    'should reject raw ASCII control %s',
    (code) => {
      expect(hasUnsafeURLCharacters('/path' + String.fromCharCode(code) + 'name')).toBe(true)
    }
  )
  it('should reject backslashes while preserving ordinary and encoded URL characters', () => {
    expect(hasUnsafeURLCharacters('/path\\name')).toBe(true)
    expect(hasUnsafeURLCharacters('/path%20name?q=é')).toBe(false)
  })
})

describe('when native navigation shares the link policy module', () => {
  it('should retain polymorphic native anchor attributes alongside runtime helpers', () => {
    type MenuLink = PolymorphicLinkProps<'a', { label: string }>
    expectTypeOf<MenuLink['href']>().toEqualTypeOf<string | undefined>()
    expectTypeOf<MenuLink['label']>().toEqualTypeOf<string>()
    expectTypeOf<'children' extends keyof MenuLink ? true : false>().toEqualTypeOf<false>()
    expect(linkHref('/places')).toBe('/places')
    expect(secureRel('_blank', undefined)).toBe('noopener noreferrer')
  })
})

describe('when navigation entries are validated before rendering', () => {
  const messages = { id: 'bad id', entry: 'bad entry' }
  const anyHref = () => true
  it('should accept unique, labelled entries the destination check accepts', () => {
    expect(() =>
      assertNavigationEntries(
        [
          { id: 'a', label: 'A', href: '/a' },
          { id: 'b', label: 'B' }
        ],
        (href) => href === undefined || linkHref(href) !== undefined,
        messages
      )
    ).not.toThrow()
  })
  it.each([null, undefined, 0, 'a', {}, { id: ' ' }, { id: 1 }])(
    'should report entry %j as an identity error instead of a TypeError',
    (entry) => {
      expect(() => assertNavigationEntries([entry], anyHref, messages)).toThrow('bad id')
    }
  )
  it('should report a repeated identity', () => {
    expect(() =>
      assertNavigationEntries(
        [
          { id: 'a', label: 'A' },
          { id: 'a', label: 'B' }
        ],
        anyHref,
        messages
      )
    ).toThrow('bad id')
  })
  it.each([{ label: '' }, { label: ' ' }, { label: 3 }, { label: 'A', href: 'javascript:x' }])(
    'should report %j as an entry error',
    (entry) => {
      expect(() =>
        assertNavigationEntries(
          [{ id: 'a', ...entry }],
          (href) => href === undefined || linkHref(href) !== undefined,
          messages
        )
      ).toThrow('bad entry')
    }
  )
})
