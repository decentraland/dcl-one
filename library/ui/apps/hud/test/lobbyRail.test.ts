import { describe, expect, it } from 'vitest'
import { RAIL_START, RailModel } from '../features/lobby/railModel'

// Cards 100 wide, 10 apart, in a viewport that shows `visible` of them.
function rail(cards: number, perPage: number, visible = perPage, arrowsMoveOneCard = false): RailModel {
  const lefts = Array.from({ length: cards }, (_, i) => i * 110)
  return new RailModel({ lefts, rights: lefts.map((l) => l + 100), viewport: visible * 110 - 10 }, cards, perPage, arrowsMoveOneCard)
}

describe('lobby rail paging', () => {
  it('has a page per cards-per-page', () => {
    expect(rail(7, 3).pageCount).toBe(3)
    expect(rail(3, 3).pageCount).toBe(1)
    expect(rail(0, 3).pageCount).toBe(0)
  })

  it('pages forward with the arrow and wraps round once the strip reached its end', () => {
    const r = rail(7, 3)
    let s = r.next(RAIL_START)
    expect(s.page).toBe(1)
    s = r.next(s)
    expect(s.page).toBe(2)
    // the last page is clamped to the end of the strip
    expect(s.offset).toBe(r.maxOffset())
    expect(r.next(s)).toEqual(RAIL_START)
  })

  it('wraps round backwards from the start', () => {
    const r = rail(7, 3)
    expect(r.previous(RAIL_START).page).toBe(2)
  })

  it('stops the wheel at both ends', () => {
    const r = rail(7, 3)
    expect(r.wheel(RAIL_START, false)).toEqual(RAIL_START)
    let s = r.wheel(r.wheel(RAIL_START, true), true)
    expect(s.page).toBe(2)
    s = r.wheel(s, true)
    expect(s.page).toBe(2)
  })

  it('moves one card at a time when asked, wrapping at the end', () => {
    const r = rail(5, 3, 3, true)
    let s = r.next(RAIL_START)
    expect(s.card).toBe(1)
    s = r.next(s)
    // card 2 reaches the end, where cards 3 and 4 share its offset: the next step wraps round
    expect(s).toEqual({ card: 2, page: 1, offset: r.maxOffset() })
    expect(r.next(s).card).toBe(0)
    expect(r.previous(RAIL_START).offset).toBe(r.maxOffset())
  })

  it('settles a drag on the nearest card, flung by the release speed', () => {
    const r = rail(7, 3)
    expect(r.dragEnd(120, 0).card).toBe(1)
    // a fast leftward release carries the strip on by up to a page
    expect(r.dragEnd(120, -2000).card).toBe(3)
    expect(r.dragEnd(120, -100000).offset).toBe(r.cardOffset(4))
  })

  it('holds a drag inside the strip', () => {
    const r = rail(7, 3)
    expect(r.dragOffset(0, 50)).toBe(0)
    expect(r.dragOffset(0, -10000)).toBe(r.maxOffset())
  })

  it('clamps a dot page to the filled pages', () => {
    const r = rail(4, 3)
    expect(r.snapTo(5).page).toBe(1)
    expect(r.snapTo(-1).page).toBe(0)
  })
})
