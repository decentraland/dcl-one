import { describe, expect, it } from 'vitest'
import { arrowStep } from '../lib/arrowKeys'

describe('arrow keys', () => {
  it('steps back and forth, wrapping around, and Home/End go to the ends', () => {
    expect(arrowStep('ArrowRight', 0, 3)).toBe(1)
    expect(arrowStep('ArrowDown', 2, 3)).toBe(0)
    expect(arrowStep('ArrowLeft', 0, 3)).toBe(2)
    expect(arrowStep('ArrowUp', 1, 3)).toBe(0)
    expect(arrowStep('Home', 2, 3)).toBe(0)
    expect(arrowStep('End', 0, 3)).toBe(2)
  })

  it('moves nothing for other keys or no items', () => {
    expect(arrowStep('Enter', 0, 3)).toBeNull()
    expect(arrowStep('ArrowDown', 0, 0)).toBeNull()
  })
})
