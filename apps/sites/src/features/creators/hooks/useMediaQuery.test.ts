// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { createElement } from 'react'
import { renderToString } from 'react-dom/server'
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
  return {
    matchMedia,
    fire: (next: boolean) => {
      media.matches = next
      listeners.forEach(l => l({ matches: next } as MediaQueryListEvent))
    }
  }
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

  it('answers the server value where matchMedia does not exist when one is given', () => {
    vi.stubGlobal('matchMedia', undefined)
    const { result } = renderHook(() => useMediaQuery('(max-width: 900px)', true))
    expect(result.current).toBe(true)
  })

  it('renders the server value on the server, whatever the viewport', () => {
    stubMatchMedia(true)
    const Probe = () => createElement('span', null, String(useMediaQuery('(max-width: 900px)')))
    expect(renderToString(createElement(Probe))).toBe('<span>false</span>')
  })

  it('stops listening once unmounted', () => {
    const { matchMedia } = stubMatchMedia(true)
    const { unmount } = renderHook(() => useMediaQuery('(max-width: 900px)'))
    const media = matchMedia.mock.results[0]?.value as { removeEventListener: unknown }
    const remove = vi.spyOn(media as { removeEventListener: () => void }, 'removeEventListener')
    unmount()
    expect(remove).toHaveBeenCalledWith('change', expect.any(Function))
  })
})
