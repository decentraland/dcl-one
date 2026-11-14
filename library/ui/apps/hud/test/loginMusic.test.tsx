import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import { useBeforeWorld, useLoginMusic } from '../features/startup/useLoginMusic'

describe('sign-in music', () => {
  let play: ReturnType<typeof vi.spyOn>
  let pause: ReturnType<typeof vi.spyOn>
  beforeEach(() => {
    setPref(PREF.loginMusicMuted, '0')
    play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
    pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {})
  })
  afterEach(() => vi.restoreAllMocks())

  it('plays until the player is in the world', () => {
    const { rerender } = renderHook(({ active }) => useLoginMusic(active), { initialProps: { active: true } })
    expect(play).toHaveBeenCalled()
    rerender({ active: false })
    expect(pause).toHaveBeenCalled()
  })

  it("doesn't come back over a trip's loading screen once in the world; a sign-out starts it over", () => {
    const { result, rerender } = renderHook(({ phase }) => useBeforeWorld(phase), { initialProps: { phase: 'login' as Parameters<typeof useBeforeWorld>[0] } })
    expect(result.current).toBe(true)
    rerender({ phase: 'entering' })
    expect(result.current).toBe(true)
    rerender({ phase: 'world' })
    expect(result.current).toBe(false)
    // a teleport or realm change shows the loader again
    rerender({ phase: 'entering' })
    expect(result.current).toBe(false)
    rerender({ phase: 'login' })
    expect(result.current).toBe(true)
  })

  it('a browser that refused to play before any interaction plays on the first click', () => {
    play.mockRejectedValueOnce(new DOMException('not allowed', 'NotAllowedError'))
    renderHook(() => useLoginMusic(true))
    const before = play.mock.calls.length
    window.dispatchEvent(new Event('pointerdown'))
    expect(play.mock.calls.length).toBe(before + 1)
  })

  it('mute stops it, is kept, and unmute plays it again', () => {
    const { result } = renderHook(() => useLoginMusic(true))
    play.mockClear()
    act(() => result.current.toggleMute())
    expect(result.current.muted).toBe(true)
    expect(pause).toHaveBeenCalled()
    expect(getPref(PREF.loginMusicMuted)).toBe('1')
    expect(renderHook(() => useLoginMusic(true)).result.current.muted).toBe(true)
    act(() => result.current.toggleMute())
    expect(getPref(PREF.loginMusicMuted)).toBe('0')
    expect(play).toHaveBeenCalled()
  })
})
