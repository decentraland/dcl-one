import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { track } from '~/lib/analytics'
import { useLiveBridge } from './useLiveBridge'

vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))

const BRIDGE = 'http://localhost:8080'

type Bridge = { version: number; bytes: number[]; down?: boolean }

/** A fake add-on bridge: `/state` answers at once (no long-poll), `/model.glb` serves the current bytes. */
function serve(bridge: Bridge) {
  const fetchMock = vi.fn(async (input: string | URL) => {
    const url = String(input)
    if (bridge.down) throw new TypeError('Failed to fetch')
    if (url.includes('/state')) {
      return new Response(JSON.stringify({ version: bridge.version, type: 'wearable', name: 'Hat', category: 'hat' }))
    }
    return new Response(new Uint8Array(bridge.bytes))
  })
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

const modelFetches = (fetchMock: ReturnType<typeof vi.fn>) =>
  fetchMock.mock.calls.filter(([url]) => String(url).endsWith('/model.glb')).length

async function settle() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(50)
  })
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(track).mockClear()
})
afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('useLiveBridge', () => {
  it('connects on mount and applies the first export', async () => {
    serve({ version: 1, bytes: [1, 2, 3] })
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    expect(result.current.status).toBe('connecting')
    await settle()
    expect(result.current.status).toBe('connected')
    expect(result.current.state).toMatchObject({ version: 1, category: 'hat' })
    expect(result.current.glb?.size).toBe(3)
    expect(result.current.pushCount).toBe(1)
    expect(track).toHaveBeenCalledWith('Live preview connect', expect.objectContaining({ long_poll: false }))
    expect(track).toHaveBeenCalledWith(
      'Live preview model',
      expect.objectContaining({ item_type: 'wearable', category: 'hat' })
    )
  })

  it('re-fetches the model only when the version moves, and ignores a re-export of the same bytes', async () => {
    const bridge: Bridge = { version: 1, bytes: [1, 2, 3] }
    const fetchMock = serve(bridge)
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    await settle()
    expect(modelFetches(fetchMock)).toBe(1)

    // Same version: polled again after the interval, no model fetch.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_100)
    })
    expect(modelFetches(fetchMock)).toBe(1)

    // New version, same bytes and metadata: fetched, then dropped as a no-op.
    bridge.version = 2
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_100)
    })
    expect(modelFetches(fetchMock)).toBe(2)
    expect(result.current.pushCount).toBe(1)

    // New version, new bytes: applied.
    bridge.version = 3
    bridge.bytes = [9, 9, 9, 9]
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_100)
    })
    expect(result.current.pushCount).toBe(2)
    expect(result.current.glb?.size).toBe(4)
    expect(result.current.state?.version).toBe(3)
  })

  it('reports an unreachable bridge, backs off, and recovers when it comes back', async () => {
    const bridge: Bridge = { version: 1, bytes: [1], down: true }
    const fetchMock = serve(bridge)
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    await settle()
    expect(result.current.status).toBe('error')
    expect(result.current.errorCode).toBe('unreachable')
    expect(track).toHaveBeenCalledWith('Live preview connect error', { error: 'unreachable' })

    const before = fetchMock.mock.calls.length
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000)
    })
    expect(fetchMock.mock.calls.length).toBe(before)

    bridge.down = false
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_100)
    })
    expect(result.current.status).toBe('connected')
    expect(result.current.pushCount).toBe(1)
  })

  it('tells a blocked local-network request apart from an unreachable bridge', async () => {
    Object.defineProperty(navigator, 'permissions', {
      value: { query: vi.fn().mockResolvedValue({ state: 'denied', addEventListener() {}, removeEventListener() {} }) },
      configurable: true
    })
    Object.defineProperty(window, 'location', { value: { hostname: 'builder.example.com' }, configurable: true })
    serve({ version: 1, bytes: [1], down: true })
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    await settle()
    expect(result.current.errorCode).toBe('permission_denied')
    expect(result.current.permission).toBe('denied')
  })

  it('refresh answers right away on a long-polling bridge instead of waiting for the next export', async () => {
    // Long-poll: a `since` request never answers while the version stands; a plain ask answers at once.
    const fetchMock = vi.fn((input: string | URL, init?: RequestInit) => {
      const url = String(input)
      if (url.includes('since=')) {
        return new Promise<Response>((_, reject) =>
          init?.signal?.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')))
        )
      }
      if (url.includes('/state')) {
        return Promise.resolve(
          new Response(JSON.stringify({ version: 1, type: 'wearable', name: 'Hat', category: 'hat' }))
        )
      }
      return Promise.resolve(new Response(new Uint8Array([1, 2, 3])))
    })
    vi.stubGlobal('fetch', fetchMock)
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    await settle()
    expect(result.current.pushCount).toBe(1)

    act(() => result.current.refresh())
    expect(result.current.isRefreshing).toBe(true)
    expect(result.current.lastCheckedAt).toBeNull()
    await settle()
    expect(result.current.isRefreshing).toBe(false)
    expect(result.current.status).toBe('connected')
    expect(result.current.pushCount).toBe(1)
    // Nothing new: the check is confirmed so the page can say "up to date".
    expect(result.current.lastCheckedAt).not.toBeNull()
  })

  it('a refresh that brings a new export applies it without claiming "up to date"', async () => {
    const bridge = { version: 1, bytes: [1, 2, 3] }
    serve(bridge)
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    await settle()
    bridge.version = 2
    bridge.bytes = [4, 5, 6]
    act(() => result.current.refresh())
    await settle()
    expect(result.current.pushCount).toBe(2)
    expect(result.current.lastCheckedAt).toBeNull()
  })

  it('stops polling on disconnect and starts over on connect', async () => {
    const fetchMock = serve({ version: 1, bytes: [1] })
    const { result } = renderHook(() => useLiveBridge(BRIDGE))
    await settle()
    act(() => result.current.disconnect())
    expect(result.current.status).toBe('disconnected')
    const before = fetchMock.mock.calls.length
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000)
    })
    expect(fetchMock.mock.calls.length).toBe(before)

    act(() => result.current.connect())
    await settle()
    expect(result.current.status).toBe('connected')
  })
})
