import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useMediaQuery } from './useMediaQuery'

type Listener = (event: MediaQueryListEvent) => void

function stubMatchMedia(matches: boolean) {
  const listeners = new Set<Listener>()
  const media = {
    matches,
    addEventListener: (_: string, listener: Listener) => listeners.add(listener),
    removeEventListener: (_: string, listener: Listener) => listeners.delete(listener)
  }
  const matchMedia = vi.fn().mockReturnValue(media)
  vi.stubGlobal('matchMedia', matchMedia)
  return { matchMedia, fire: (next: boolean) => listeners.forEach(l => l({ matches: next } as MediaQueryListEvent)) }
}

afterEach(() => vi.unstubAllGlobals())

describe('useMediaQuery', () => {
  it('strips the @media prefix and follows the query as the viewport changes', () => {
    const { matchMedia, fire } = stubMatchMedia(true)
    const { result } = renderHook(() => useMediaQuery('@media (max-width: 900px)'))
    expect(matchMedia).toHaveBeenCalledWith('(max-width: 900px)')
    expect(result.current).toBe(true)
    act(() => fire(false))
    expect(result.current).toBe(false)
  })

  it('is false where matchMedia does not exist', () => {
    vi.stubGlobal('matchMedia', undefined)
    const { result } = renderHook(() => useMediaQuery('@media (max-width: 900px)'))
    expect(result.current).toBe(false)
  })
})
