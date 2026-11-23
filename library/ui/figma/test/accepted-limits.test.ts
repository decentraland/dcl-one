import { readFileSync } from 'node:fs'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { componentRegistryEntries } from '../component-generation.mjs'

const ui = path.resolve(import.meta.dirname, '../..')
const MARKER = '<!-- navbar-accepted-limits -->'

const INVALID_LIMIT = 'Invalid or duplicate accepted limit of component NavBar'
const NEEDS_NAME = 'A component registry entry object needs a component name'
const LIMIT_COUNT = 'A component registry entry object needs between 1 and 10 limits: NavBar'

describe('when checking the accepted limits of generated public components', () => {
  // NavBar, the last generated public component, is hand-written: its limits live in docs/ui-components.md.
  let promotion: unknown
  beforeEach(() => {
    promotion = JSON.parse(readFileSync(path.join(ui, 'components/promotion.json'), 'utf8'))
  })

  it('should record no limits in the component registry', () => {
    expect(componentRegistryEntries(promotion).limits).toEqual({})
  })

  it('should keep no accepted-limits list in the README', () => {
    expect(readFileSync(path.join(ui, 'README.md'), 'utf8')).not.toContain(MARKER)
  })
})

describe('componentRegistryEntries', () => {
  const limit = { id: 'logo-size', text: 'The logo is pending.' }

  describe('when public entries mix names and entries with limits', () => {
    let registry: unknown
    beforeEach(() => {
      registry = {
        components: ['Button', { name: 'NavBar', limits: [limit] }],
        internal: ['Menu']
      }
    })

    it('should accept names and public entries with limits', () => {
      expect(componentRegistryEntries(registry)).toEqual({
        components: ['Button', 'NavBar'],
        internal: ['Menu'],
        limits: { NavBar: [limit] }
      })
    })
  })

  describe('when a public entry is invalid', () => {
    it.each([
      [
        'a duplicate id',
        { name: 'NavBar', limits: [limit, { ...limit, text: 'Other.' }] },
        INVALID_LIMIT
      ],
      ['an empty text', { name: 'NavBar', limits: [{ id: 'a', text: '' }] }, INVALID_LIMIT],
      ['padded text', { name: 'NavBar', limits: [{ id: 'a', text: ' padded' }] }, INVALID_LIMIT],
      [
        'an over-long text',
        { name: 'NavBar', limits: [{ id: 'a', text: 'x'.repeat(301) }] },
        INVALID_LIMIT
      ],
      [
        'a link',
        { name: 'NavBar', limits: [{ id: 'a', text: 'see https://example.com' }] },
        INVALID_LIMIT
      ],
      [
        'a control character',
        { name: 'NavBar', limits: [{ id: 'a', text: 'line\nbreak' }] },
        INVALID_LIMIT
      ],
      [
        'a C1 control',
        { name: 'NavBar', limits: [{ id: 'a', text: 'next\u0085line' }] },
        INVALID_LIMIT
      ],
      [
        'a bidi override',
        { name: 'NavBar', limits: [{ id: 'a', text: 'abc\u202Edef' }] },
        INVALID_LIMIT
      ],
      [
        'a zero-width space',
        { name: 'NavBar', limits: [{ id: 'a', text: 'zero\u200Bwidth' }] },
        INVALID_LIMIT
      ],
      [
        'a paragraph separator',
        { name: 'NavBar', limits: [{ id: 'a', text: 'a\u2029b' }] },
        INVALID_LIMIT
      ],
      [
        'a www link',
        { name: 'NavBar', limits: [{ id: 'a', text: 'see www.example.org' }] },
        INVALID_LIMIT
      ],
      [
        'a mailto link',
        { name: 'NavBar', limits: [{ id: 'a', text: 'mailto:someone' }] },
        INVALID_LIMIT
      ],
      ['markup', { name: 'NavBar', limits: [{ id: 'a', text: 'an <img> tag' }] }, INVALID_LIMIT],
      ['a name that is not a string', { name: 5, limits: [limit] }, NEEDS_NAME],
      ['no name', { limits: [limit] }, NEEDS_NAME],
      ['a name that is not an identifier', { name: '../NavBar', limits: [limit] }, NEEDS_NAME],
      [
        'more than ten limits',
        {
          name: 'NavBar',
          limits: Array.from({ length: 11 }, (_, i) => ({ id: `l${i}`, text: 'Pending.' }))
        },
        LIMIT_COUNT
      ],
      [
        'an id that is not kebab-case',
        { name: 'NavBar', limits: [{ id: 'Logo_Size', text: 'x.' }] },
        INVALID_LIMIT
      ],
      ['an empty limits list', { name: 'NavBar', limits: [] }, LIMIT_COUNT],
      [
        'an unknown key',
        { name: 'NavBar', limits: [limit], extra: true },
        'Invalid component registry entry'
      ],
      ['an unknown limit key', { name: 'NavBar', limits: [{ ...limit, url: 'x' }] }, INVALID_LIMIT]
    ])('should refuse %s', (_label, entry, message) => {
      expect(() => componentRegistryEntries({ components: [entry] })).toThrow(message)
    })
  })

  // The nearest false positives of the stricter text rule and the cap.
  describe('when a limit text is plain', () => {
    it.each([
      [
        'ordinary punctuation and units',
        'The logo is 60px on desktop; alignment (40px) is pending.'
      ],
      ['a colon that is not a scheme', 'Note: the avatar is a placeholder.'],
      ['a non-ASCII letter', 'Résumé views are pending.']
    ])('should accept %s', (_label, text) => {
      expect(
        componentRegistryEntries({ components: [{ name: 'NavBar', limits: [{ id: 'a', text }] }] })
          .limits.NavBar
      ).toEqual([{ id: 'a', text }])
    })
  })

  describe('when an entry has exactly ten limits', () => {
    let limits: { id: string; text: string }[]
    beforeEach(() => {
      limits = Array.from({ length: 10 }, (_, i) => ({ id: `l${i}`, text: 'Pending.' }))
    })

    it('should accept all ten limits', () => {
      expect(
        componentRegistryEntries({ components: [{ name: 'NavBar', limits }] }).limits.NavBar
      ).toHaveLength(10)
    })
  })

  describe('when a name is registered twice', () => {
    it.each([
      ['a public name twice', { components: ['NavBar', { name: 'NavBar', limits: [limit] }] }],
      ['a name both public and internal', { components: ['NavBar'], internal: ['NavBar'] }],
      ['a name twice in another case', { components: ['NavBar', 'navbar'] }]
    ])('should refuse %s', (_label, registry) => {
      expect(() => componentRegistryEntries(registry)).toThrow('Duplicate component registry name')
    })
  })

  // The nearest false positive: distinct names, one with limits, are kept apart.
  describe('when names are distinct and one has limits', () => {
    let registry: unknown
    beforeEach(() => {
      registry = { components: ['Button', { name: 'NavBar', limits: [limit] }] }
    })

    it('should accept distinct names', () => {
      expect(componentRegistryEntries(registry).components).toEqual(['Button', 'NavBar'])
    })
  })

  describe('when an internal entry carries limits', () => {
    let registry: unknown
    beforeEach(() => {
      registry = { components: [], internal: [{ name: 'NavBar', limits: [limit] }] }
    })

    it('should refuse the internal entry', () => {
      expect(() => componentRegistryEntries(registry)).toThrow(
        'Internal component registry entries are names only'
      )
    })
  })
})
