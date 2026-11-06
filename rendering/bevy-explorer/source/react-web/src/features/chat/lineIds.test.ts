import { describe, it, expect } from 'vitest'
import { risingIds } from './lineIds'

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
