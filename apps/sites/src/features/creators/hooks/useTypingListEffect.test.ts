import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useTypingListEffect } from './useTypingListEffect'

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

// Each tick reschedules itself from an effect, which React flushes after `act`, so advance one tick per act.
function ticks(count: number, ms = 100) {
  for (let i = 0; i < count; i++) {
    act(() => {
      vi.advanceTimersByTime(ms)
    })
  }
}

describe('useTypingListEffect', () => {
  it('types the first word letter by letter, holds it ~2s, erases it and moves on to the next word', () => {
    const { result } = renderHook(() => useTypingListEffect(['Ab', 'Xyz']))
    expect(result.current).toBe('A')

    ticks(1)
    expect(result.current).toBe('Ab')

    // Held once when the word completes and again on the flip to erase, as on sites.
    ticks(1, 1000)
    expect(result.current).toBe('Ab')
    ticks(1, 999)
    expect(result.current).toBe('Ab')
    ticks(1, 1)
    expect(result.current).toBe('A')

    ticks(1)
    expect(result.current).toBe('X')
    ticks(2)
    expect(result.current).toBe('Xyz')
  })

  it('renders nothing and schedules no ticks for an empty list', () => {
    const { result } = renderHook(() => useTypingListEffect([]))
    expect(result.current).toBe('')
    ticks(3, 1000)
    expect(result.current).toBe('')
  })

  it('wraps around to the first word after the last', () => {
    const { result } = renderHook(() => useTypingListEffect(['A', 'B']))
    expect(result.current).toBe('A')
    ticks(2, 1000)
    expect(result.current).toBe('B')
    ticks(2, 1000)
    expect(result.current).toBe('A')
  })
})
