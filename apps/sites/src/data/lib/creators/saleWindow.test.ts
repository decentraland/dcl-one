import { describe, expect, it } from 'vitest'
import { endOfDay, startOfDay, windowOf } from './saleWindow'

const at = (y: number, m: number, d: number, h = 0, min = 0) => new Date(y, m, d, h, min).getTime()
const NOW = at(2026, 9, 19, 15, 30)

describe('when deciding the window a discount is signed with', () => {
  it('should run a preset from now for its length', () => {
    expect(windowOf({ kind: 'preset', key: '3d', hours: 72 }, NOW)).toEqual({
      startsAtMs: undefined,
      endsAtMs: NOW + 72 * 60 * 60 * 1000
    })
  })

  // Its first midnight has already gone by: signing a start in the past would be refused.
  it('should start a range that begins today now, and end it at the end of its last day', () => {
    expect(windowOf({ kind: 'range', from: at(2026, 9, 19), to: at(2026, 9, 21) }, NOW)).toEqual({
      startsAtMs: undefined,
      endsAtMs: at(2026, 9, 21, 23, 59) + 59_999
    })
  })

  it('should schedule a range that begins on a later day from that day midnight', () => {
    expect(windowOf({ kind: 'range', from: at(2026, 9, 22, 10), to: at(2026, 9, 25) }, NOW)).toEqual({
      startsAtMs: at(2026, 9, 22),
      endsAtMs: endOfDay(at(2026, 9, 25))
    })
  })

  it('should bound a day by its local midnight and its last millisecond', () => {
    expect(startOfDay(NOW)).toBe(at(2026, 9, 19))
    expect(endOfDay(NOW)).toBe(at(2026, 9, 20) - 1)
  })
})
