import { describe, expect, it } from 'vitest'
import { holeClip } from '../lib/holeClip'

const points = (clipPath: unknown): string[] => String(clipPath).replace(/^polygon\(evenodd, |\)$/g, '').split(', ')

describe('holeClip', () => {
  it('cuts a square-cornered hole out of the whole box', () => {
    expect(holeClip({ x: 10, y: 20, width: 30, height: 40 }).clipPath).toBe(
      'polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, 10px 20px, 40px 20px, 40px 60px, 10px 60px, 10px 20px, 0 0)'
    )
  })

  it('rounds only the corners asked for, keeping the hole inside its rect', () => {
    const hole = points(holeClip({ x: 0, y: 0, width: 100, height: 100 }, { topLeft: 20 }).clipPath).slice(5, -1)
    expect(hole[0]).toBe('0.00px 20.00px')
    expect(hole).toContain('20.00px 0.00px')
    expect(hole).not.toContain('0px 0px')
    expect(hole).toContain('100px 0px')
    expect(hole).toContain('100px 100px')
    expect(hole).toContain('0px 100px')
  })
})
