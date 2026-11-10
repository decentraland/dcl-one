import { describe, it, expect } from 'vitest'
import { risingIds, rowKeys } from '@hud/features/chat/lineIds'

describe('risingIds', () => {
  const counter = (start: number) => {
    let n = start
    return () => n++
  }

  it('keeps the ids of lines already in order', () => {
    const lines = [{ id: 0 }, { id: 3 }, { id: 4 }]
    expect(risingIds(lines, counter(10))).toEqual([{ id: 0 }, { id: 3 }, { id: 4 }])
  })

  it('gives a higher id to a line placed below a higher one, and keeps the rest', () => {
    // a live line took id 2 while the rows behind it got 3 and 4
    const lines = [{ id: 0 }, { id: 1 }, { id: 3 }, { id: 4 }, { id: 2 }]
    expect(risingIds(lines, counter(5))).toEqual([{ id: 0 }, { id: 1 }, { id: 3 }, { id: 4 }, { id: 5 }])
  })

  it('gives two lines with the same id distinct, rising ids', () => {
    const lines = [{ id: 0, n: 'a' }, { id: 0, n: 'b' }]
    const out = risingIds(lines, counter(1))
    expect(out).toEqual([{ id: 0, n: 'a' }, { id: 1, n: 'b' }])
  })

  it('does not change its input', () => {
    const lines = [{ id: 2 }, { id: 1 }]
    risingIds(lines, counter(3))
    expect(lines).toEqual([{ id: 2 }, { id: 1 }])
  })
})

describe('rowKeys', () => {
  it('keys a row on its message id', () => {
    expect(rowKeys(['0xa:1', '0xb:2'])).toEqual(['0xa:1#0', '0xb:2#0'])
  })

  it('tells rows repeating a message id apart by their order', () => {
    expect(rowKeys(['0xa:1', '0xa:1', '0xb:2', '0xa:1'])).toEqual(['0xa:1#0', '0xa:1#1', '0xb:2#0', '0xa:1#2'])
  })

  it('keys a row without a message id on its position', () => {
    expect(rowKeys(['', '0xa:1', ''])).toEqual(['#0', '0xa:1#0', '#2'])
  })
})
