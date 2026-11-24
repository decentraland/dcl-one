import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useMapGridAge } from '../features/map/useMapGridAge'

const MINUTE = 60_000
const NOW = Date.UTC(2026, 10, 24, 12, 0, 0)
const BASE = 'https://map.test'

/** What the map API answers: its grid's checkedAt, none, an error status or no answer at all. */
let answer: { checkedAt: number } | { status: number } | 'offline' | 'none'
const answering = async (_url: string, _init?: RequestInit): Promise<Response> => {
  if (answer === 'offline') throw new TypeError('Failed to fetch')
  if (answer === 'none') return Response.json({ lastUpdatedAt: NOW - 600 * MINUTE })
  if ('status' in answer) return new Response(null, { status: answer.status })
  return Response.json({ lastUpdatedAt: NOW - 600 * MINUTE, checkedAt: answer.checkedAt })
}
const fetch = vi.fn(answering)

const advance = (ms: number): Promise<void> =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })

beforeEach(() => {
  vi.useFakeTimers({ now: NOW })
  vi.stubGlobal('fetch', fetch)
  fetch.mockReset()
  fetch.mockImplementation(answering)
  answer = 'none'
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  setVisibility('visible')
})

function setVisibility(state: 'visible' | 'hidden'): void {
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => state })
  document.dispatchEvent(new Event('visibilitychange'))
}

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

  it("measures the age on the server's clock, so a device clock ahead shows no stale label", async () => {
    // This device runs six minutes ahead of the server, whose grid was checked a minute ago.
    vi.setSystemTime(NOW + 6 * MINUTE)
    fetch.mockImplementation(async () =>
      Response.json({ checkedAt: NOW - MINUTE }, { headers: { date: new Date(NOW).toUTCString() } })
    )
    const { result } = renderHook(() => useMapGridAge(BASE, true))
    await advance(0)
    expect(result.current).toBeNull()
  })

  it('gives up a read still pending when the next starts, so a late answer never lands', async () => {
    let lateAnswer: (response: Response) => void = () => {}
    fetch.mockImplementationOnce(() => new Promise<Response>((resolve) => (lateAnswer = resolve)))
    answer = { checkedAt: NOW - 30 * MINUTE }
    const { result } = renderHook(() => useMapGridAge(BASE, true))
    await advance(MINUTE)
    expect(fetch.mock.calls[0]?.[1]?.signal?.aborted).toBe(true)
    expect(result.current).toBe('Updated 31 min ago')
    lateAnswer(Response.json({ checkedAt: NOW + MINUTE }))
    await advance(0)
    expect(result.current).toBe('Updated 31 min ago')
  })

  it('reads nothing while the page is hidden, and reads at once when it is shown again', async () => {
    answer = { checkedAt: NOW - 30 * MINUTE }
    const { result } = renderHook(() => useMapGridAge(BASE, true))
    await advance(0)
    expect(result.current).toBe('Updated 30 min ago')
    act(() => setVisibility('hidden'))
    expect(result.current).toBeNull()
    await advance(5 * MINUTE)
    expect(fetch).toHaveBeenCalledOnce()
    act(() => setVisibility('visible'))
    await advance(0)
    expect(fetch).toHaveBeenCalledTimes(2)
    expect(result.current).toBe('Updated 35 min ago')
  })
})
