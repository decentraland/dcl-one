import { describe, expect, it } from 'vitest'
import type { RawInteraction } from '../ir/raw-components.js'
import { stateMotion } from './motion.js'

const hoverTo = (transition: RawInteraction['transition']): RawInteraction[] => [
  { trigger: 'ON_HOVER', destinationId: 'hover', transition }
]

describe('stateMotion', () => {
  describe('when the rest variant smart-animates to its hover variant while hovering', () => {
    it('should return the duration in milliseconds and the CSS curve', () => {
      expect(
        stateMotion(
          hoverTo({ type: 'SMART_ANIMATE', duration: 0.30000001192092896, easing: 'EASE_OUT' }),
          'hover',
          'hover'
        )
      ).toEqual({ motion: { duration: 300, easing: 'ease-out' } })
    })

    it('should write a custom curve as cubic-bezier', () => {
      expect(
        stateMotion(
          hoverTo({
            type: 'SMART_ANIMATE',
            duration: 0.2,
            easing: 'CUSTOM_CUBIC_BEZIER',
            bezier: [0.4, 0, 0.2, 1]
          }),
          'hover',
          'hover'
        ).motion
      ).toEqual({ duration: 200, easing: 'cubic-bezier(0.4, 0, 0.2, 1)' })
    })
  })

  describe('when there is nothing to animate', () => {
    it('should return nothing without an interaction, an instant one, another trigger or another destination', () => {
      expect(stateMotion(undefined, 'hover', 'hover')).toEqual({})
      expect(stateMotion(hoverTo(undefined), 'hover', 'hover')).toEqual({})
      expect(
        stateMotion(
          [
            {
              trigger: 'ON_CLICK',
              destinationId: 'hover',
              transition: { type: 'SMART_ANIMATE', duration: 0.2, easing: 'LINEAR' }
            }
          ],
          'hover',
          'hover'
        )
      ).toEqual({})
      expect(
        stateMotion(
          hoverTo({ type: 'SMART_ANIMATE', duration: 0.2, easing: 'LINEAR' }),
          'hover',
          'pressed-variant'
        )
      ).toEqual({})
    })
  })

  describe('when the motion has no CSS equivalent', () => {
    it('should report a spring curve or a non smart-animate transition instead of guessing', () => {
      expect(
        stateMotion(
          hoverTo({ type: 'SMART_ANIMATE', duration: 0.2, easing: 'GENTLE' }),
          'hover',
          'hover'
        )
      ).toEqual({ issue: 'unsupported-easing:GENTLE' })
      expect(
        stateMotion(
          hoverTo({ type: 'DISSOLVE', duration: 0.2, easing: 'LINEAR' }),
          'hover',
          'hover'
        )
      ).toEqual({ issue: 'unsupported-transition:DISSOLVE' })
    })
  })
})
