import { describe, expect, it } from 'vitest'
import { snapshotSource } from '../../test/snapshot.js'
import { ColorResolver } from './colors.js'

async function resolver() {
  return new ColorResolver(await snapshotSource().getTokenCollections())
}

describe('ColorResolver', () => {
  describe('when a paint has neither a known token nor a colour', () => {
    it.each([
      ['a gradient passed as a solid colour', { opacity: 1 }],
      ['an unknown token without a colour', { token: 'Primitives/Missing' }]
    ])('should report %s as unresolved instead of drawing it black', async (_, paint) => {
      const colors = await resolver()

      expect(colors.resolve(paint)).toEqual({ css: 'transparent', via: 'unresolved' })
    })
  })

  describe('when a paint is bound to a token', () => {
    it('should reference the CSS variable and never the raw hex', async () => {
      const colors = await resolver()

      expect(colors.resolve({ hex: '#fcfcfc', token: 'Primitives/Neutral/White' })).toEqual({
        css: 'var(--dcl-neutral-white)',
        via: 'token',
        token: 'Primitives/Neutral/White'
      })
    })

    it('should not apply the paint opacity twice when the token already carries it', async () => {
      const colors = await resolver()

      const result = colors.resolve({
        hex: '#ffffff',
        opacity: 0.1,
        token: 'Primitives/Alpha/White 10'
      })

      expect(result.css).toBe('var(--dcl-alpha-white-10)')
    })

    it('should mix in the paint opacity when it differs from an opaque token', async () => {
      const colors = await resolver()

      const result = colors.resolve({
        hex: '#fcfcfc',
        opacity: 0.5,
        token: 'Primitives/Neutral/White'
      })

      expect(result.css).toBe('color-mix(in srgb, var(--dcl-neutral-white) 50%, transparent)')
    })
  })

  describe('when a paint is not bound but equals a palette colour', () => {
    it('should reuse the matching token and say it was matched by value', async () => {
      const colors = await resolver()

      expect(colors.resolve({ hex: '#cfcdd4' })).toMatchObject({
        css: 'var(--dcl-neutral-gray)',
        via: 'value-match'
      })
    })
  })

  describe('when a paint is not bound and matches nothing', () => {
    it('should fall back to the literal colour', async () => {
      const colors = await resolver()

      expect(colors.resolve({ hex: '#123456' })).toEqual({ css: '#123456', via: 'literal' })
      expect(colors.resolve({ hex: '#123456', opacity: 0.5 }).css).toBe('rgba(18, 52, 86, 0.5)')
    })
  })
})
