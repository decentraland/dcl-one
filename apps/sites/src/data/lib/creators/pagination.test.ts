import { describe, it, expect } from 'vitest'
import { pageRangeLabel, pageWindow } from './pagination'

describe('pageWindow', () => {
  it('lists every page when there are five or fewer', () => {
    expect(pageWindow(1, 1)).toEqual([1])
    expect(pageWindow(2, 3)).toEqual([1, 2, 3])
    expect(pageWindow(5, 5)).toEqual([1, 2, 3, 4, 5])
  })

  it('centres the window on the current page', () => {
    expect(pageWindow(5, 9)).toEqual([3, 4, 5, 6, 7])
  })

  it('clamps the window at both ends', () => {
    expect(pageWindow(1, 9)).toEqual([1, 2, 3, 4, 5])
    expect(pageWindow(9, 9)).toEqual([5, 6, 7, 8, 9])
  })
})

describe('pageRangeLabel', () => {
  it('spans the first page from 1 to the page size', () => {
    expect(pageRangeLabel(1, 20, 20)).toBe('1-20')
  })

  it('offsets later pages and stops at the last returned item', () => {
    expect(pageRangeLabel(3, 20, 7)).toBe('41-47')
  })

  it('collapses a single item to one index and an empty page to 0', () => {
    expect(pageRangeLabel(1, 8, 1)).toBe('1')
    expect(pageRangeLabel(1, 8, 0)).toBe('0')
  })
})
