import { describe, expect, it } from 'vitest'
import type { TokenCollection } from './ir/schemas.js'
import { foundationTokens } from './review-manifest.js'

const collections: TokenCollection[] = [
  {
    name: 'Primitives',
    modes: ['Value'],
    variables: [
      { name: 'Primary/Primary', type: 'COLOR', values: { Value: { hex: '#ff2d55', alpha: 1 } } },
      {
        name: 'Primary/Primary blue TEST',
        type: 'COLOR',
        values: { Value: { hex: '#0400ff', alpha: 1 } }
      }
    ]
  },
  {
    name: 'Semantic',
    modes: ['Dark'],
    variables: [
      {
        name: 'action/primary',
        type: 'COLOR',
        values: { Dark: { alias: 'Primary/Primary', collection: 'Primitives' } }
      }
    ]
  }
]

describe('foundationTokens', () => {
  describe('when no primitive is excluded', () => {
    it('should list every saved variable with its CSS variable', () => {
      expect(foundationTokens(collections, []).map((token) => token.cssName)).toEqual([
        '--dcl-primary-primary',
        '--dcl-primary-primary-blue-test',
        '--dcl-action-primary'
      ])
    })
  })

  describe('when a primitive is excluded', () => {
    it('should not list a swatch for a CSS variable the tokens no longer define', () => {
      const tokens = foundationTokens(collections, ['Primitives/Primary/Primary blue TEST'])

      expect(tokens.map((token) => token.cssName)).toEqual([
        '--dcl-primary-primary',
        '--dcl-action-primary'
      ])
      expect(tokens[1]).toMatchObject({ value: { hex: '#ff2d55', alpha: 1 } })
    })

    it('should fail closed on an unsupported exclusion', () => {
      expect(() => foundationTokens(collections, ['Primitives/Primary/Primary'])).toThrow(
        'is not a supported exclusion'
      )
    })
  })
})
