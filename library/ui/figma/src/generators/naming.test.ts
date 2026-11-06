import { describe, expect, it } from 'vitest'
import { assertIdentifier, camelCase, kebabCase, pascalCase, uniqueName } from './naming.js'

describe('naming', () => {
  describe('when converting figma names', () => {
    it('should camelCase words and digits', () => {
      expect(camelCase('Primary Dark 1')).toBe('primaryDark1')
      expect(camelCase('text-on-action')).toBe('textOnAction')
    })

    it('should PascalCase category paths and drop separators', () => {
      expect(pascalCase('general/home2')).toBe('GeneralHome2')
      expect(pascalCase('navbar/_deprecated/credits-old')).toBe('NavbarDeprecatedCreditsOld')
    })

    it('should kebab-case token names for CSS variables', () => {
      expect(kebabCase('Neutrals/Onix Dark 1')).toBe('neutrals-onix-dark-1')
    })
  })

  describe('when two names resolve to the same identifier', () => {
    it('should suffix the later ones and track them as used', () => {
      const used = new Set<string>()

      expect([
        uniqueName('GeneralHome2', used),
        uniqueName('GeneralHome2', used),
        uniqueName('GeneralHome2', used)
      ]).toEqual(['GeneralHome2', 'GeneralHome2_2', 'GeneralHome2_3'])
    })
  })
})

describe('when validating generated identifiers and import path segments', () => {
  it.each(['3DCard', 'social media', 'arrow-icons', 'default', '../escape', ''])(
    'should reject %j',
    (value) => {
      expect(() => assertIdentifier(value)).toThrow('Invalid generated TypeScript identifier')
    }
  )
})
