import { describe, expect, it } from 'vitest'
import type { ComponentSpec } from './ir/components.js'
import { supersessionCandidates } from './runtime.js'

const spec = (name: string, extra: Partial<ComponentSpec> = {}) =>
  ({ name, page: 'Buttons', needsManual: [], ...extra }) as ComponentSpec

describe('supersessionCandidates', () => {
  describe('when a ready page generates an eligible component that promotion.include does not name', () => {
    it('should leave the hand-written mapping of that name to be validated', () => {
      expect(supersessionCandidates([spec('Button')], ['NavBar'], ['NavBar'])).toEqual(new Map())
    })
  })

  describe('when promotion.include is absent', () => {
    it('should supersede nothing', () => {
      expect(supersessionCandidates([spec('Button')], undefined, [])).toEqual(new Map())
    })
  })

  describe('when promotion.include names an eligible component not promoted yet', () => {
    it('should report it as superseding with no blockers', () => {
      expect(supersessionCandidates([spec('Button')], ['Button'], [])).toEqual(
        new Map([['Button', []]])
      )
    })
  })

  describe('when promotion.include names a component promotion still excludes', () => {
    it('should report the reasons it stays pending', () => {
      expect(
        supersessionCandidates(
          [spec('Button', { needsManual: ['unresolved-instance:Icon'] })],
          ['Button'],
          []
        )
      ).toEqual(new Map([['Button', ['unresolved-instance:Icon']]]))
    })
  })

  describe('when the named component was already promoted', () => {
    it('should not report it, since its mapping is the synced one', () => {
      expect(supersessionCandidates([spec('NavBar')], ['NavBar'], ['NavBar'])).toEqual(new Map())
    })
  })
})
