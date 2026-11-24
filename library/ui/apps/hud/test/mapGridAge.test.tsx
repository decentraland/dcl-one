import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useMapGridAge } from '../features/map/useMapGridAge'

const MINUTE = 60_000
const NOW = Date.UTC(2026, 10, 24, 12, 0, 0)
const BASE = 'https://map.test'

/** What the map API answers: its grid's checkedAt, none, an error status or no answer at all. */
let answer: { checkedAt: number } | { status: number } | 'offline' | 'none'
const fetch = vi.fn(async (_url: string, _init?: RequestInit): Promise<Response> => {
  if (answer === 'offline') throw new TypeError('Failed to fetch')
  if (answer === 'none') return Response.json({ lastUpdatedAt: NOW - 600 * MINUTE })
  if ('status' in answer) return new Response(null, { status: answer.status })
  return Response.json({ lastUpdatedAt: NOW - 600 * MINUTE, checkedAt: answer.checkedAt })
})

const advance = (ms: number): Promise<void> =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })

beforeEach(() => {
  vi.useFakeTimers({ now: NOW })
  vi.stubGlobal('fetch', fetch)
  fetch.mockClear()
  answer = 'none'
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('the parcel map age', () => {
  it("labels a grid the map API last found current over five minutes ago", async () => {
    answer = { checkedAt: NOW - 12 * MINUTE }
    const { result } = renderHook(() => useMapGridAge(`${BASE}/`, true))
    await advance(0)
    expect(fetch).toHaveBeenCalledWith(`${BASE}/v2/tiles/info`, expect.objectContaining({ signal: expect.any(AbortSignal) }))
    expect(result.current).toBe('Updated 12 min ago')
  })

  it('shows no label for a fresher grid, a grid without checkedAt or a map API that cannot serve one', async () => {
    for (const next of [{ checkedAt: NOW - 4 * MINUTE }, 'none', { status: 503 }] as const) {
      answer = next
      const { result, unmount } = renderHook(() => useMapGridAge(BASE, true))
      await advance(0)
      expect(result.current).toBeNull()
      unmount()
    }
    expect(fetch).toHaveBeenCalledTimes(3)
  })

  it('reads the age every minute while the map is shown, and keeps ageing the last answer when a read fails', async () => {
    answer = { checkedAt: NOW - 2 * MINUTE }
    const { result } = renderHook(() => useMapGridAge(BASE, true))
    await advance(0)
    expect(result.current).toBeNull()

    answer = 'offline'
    await advance(4 * MINUTE)
    expect(fetch).toHaveBeenCalledTimes(5)
    expect(result.current).toBe('Updated 6 min ago')

    // The server found its grid current again.
    answer = { checkedAt: NOW + 4 * MINUTE }
    await advance(MINUTE)
    expect(result.current).toBeNull()
  })

  it('clears the label once the map API stops serving a grid', async () => {
    answer = { checkedAt: NOW - 30 * MINUTE }
    const { result } = renderHook(() => useMapGridAge(BASE, true))
    await advance(0)
    expect(result.current).toBe('Updated 30 min ago')
    answer = { status: 503 }
    await advance(MINUTE)
    expect(result.current).toBeNull()
  })

  it('reads nothing while the map is hidden, and stops reading when it is hidden', async () => {
    answer = { checkedAt: NOW - 30 * MINUTE }
    const { result, rerender } = renderHook(({ shown }) => useMapGridAge(BASE, shown), { initialProps: { shown: false } })
    await advance(5 * MINUTE)
    expect(fetch).not.toHaveBeenCalled()
    expect(result.current).toBeNull()

    rerender({ shown: true })
    await advance(0)
    expect(result.current).toBe('Updated 35 min ago')

    rerender({ shown: false })
    expect(result.current).toBeNull()
    expect(fetch.mock.calls[0]?.[1]?.signal?.aborted).toBe(true)
    await advance(5 * MINUTE)
    expect(fetch).toHaveBeenCalledOnce()
  })

  it('shows no label from an earlier showing until the map API answers again', async () => {
    answer = { checkedAt: NOW - 30 * MINUTE }
    const { result, rerender } = renderHook(({ shown }) => useMapGridAge(BASE, shown), { initialProps: { shown: true } })
    await advance(0)
    expect(result.current).toBe('Updated 30 min ago')
    rerender({ shown: false })

    let release: () => void = () => {}
    fetch.mockImplementationOnce(
      () =>
        new Promise<Response>((resolve) => {
          release = () => resolve(Response.json({ checkedAt: Date.now() - MINUTE }))
        })
    )
    rerender({ shown: true })
    expect(result.current).toBeNull()
    release()
    await advance(0)
    expect(result.current).toBeNull()
  })
})
