import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import { fetchContentSize } from '~/lib/builder'
import { ItemFileError } from '~/lib/itemFiles'
import { MAX_CONCURRENT_MEASUREMENTS, measureItemBytes } from '~/lib/itemSize'
import { ItemType, type Item } from '~/lib/items'
import { captureError } from '~/lib/monitoring'
import { itemSizeDeps, useCatalystItemSizes, useMeasuredBytes } from './useItemSize'

vi.mock('~/lib/builder', () => ({ fetchContent: vi.fn(), fetchContentSize: vi.fn() }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))

const STORED: Record<string, number> = { bafmodel: 2_000_000, bafthumb: 200_000, bafimage: 50_000 }

function item(id: string, contents: Record<string, string>): Item {
  return {
    id,
    name: id,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    rarity: 'epic',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents,
    createdAt: 1,
    updatedAt: 1
  }
}

const hat = item('hat', { 'male/hat.glb': 'bafmodel', 'thumbnail.png': 'bafthumb', 'image.png': 'bafimage' })

function wrapperWith(client = new QueryClient()) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
}

beforeEach(() => {
  vi.mocked(captureError).mockReset()
  vi.mocked(fetchContentSize).mockReset()
  vi.mocked(fetchContentSize).mockImplementation(async hash => {
    if (!(hash in STORED)) throw new Error(`storage down for ${hash}`)
    return STORED[hash]
  })
})

describe('useMeasuredBytes', () => {
  it('measures what the build resolves to and measures again when an input changes', async () => {
    const { result, rerender } = renderHook(
      ({ built }: { built: Item }) => useMeasuredBytes(async () => ({ item: built }), [built], 'test'),
      { wrapper: wrapperWith(), initialProps: { built: hat } }
    )
    await waitFor(() => expect(result.current).toBe(2_250_000))

    const lighter = { ...hat, contents: { ...hat.contents, 'male/hat.glb': 'bafthumb' } }
    rerender({ built: lighter })
    await waitFor(() => expect(result.current).toBe(250_000))
  })

  it('keeps its result while the inputs are the same objects', async () => {
    const build = vi.fn(async () => ({ item: hat }))
    const { result, rerender } = renderHook(() => useMeasuredBytes(build, [hat, 'male'], 'test'), {
      wrapper: wrapperWith()
    })
    await waitFor(() => expect(result.current).toBe(2_250_000))
    rerender()
    expect(result.current).toBe(2_250_000)
    expect(build).toHaveBeenCalledTimes(1)
  })

  it('measures nothing without a build', () => {
    const { result } = renderHook(() => useMeasuredBytes(null, ['none'], 'test'), { wrapper: wrapperWith() })
    expect(result.current).toBeNull()
    expect(fetchContentSize).not.toHaveBeenCalled()
  })

  it('is unknown, unreported, while the build itself refuses the files', async () => {
    const build = vi.fn(async () => Promise.reject(new ItemFileError('invalid_representation')))
    const { result } = renderHook(() => useMeasuredBytes(build, ['refused'], 'test'), { wrapper: wrapperWith() })
    await waitFor(() => expect(build).toHaveBeenCalled())
    expect(result.current).toBeNull()
    expect(captureError).not.toHaveBeenCalled()
  })

  it('reports a failed measurement and stays unknown', async () => {
    const broken = item('broken', {
      'male/hat.glb': 'bafmissing',
      'thumbnail.png': 'bafthumb',
      'image.png': 'bafimage'
    })
    const { result } = renderHook(() => useMeasuredBytes(async () => ({ item: broken }), [broken], 'test'), {
      wrapper: wrapperWith()
    })
    await waitFor(() =>
      expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'test', step: 'measure_size' })
    )
    expect(result.current).toBeNull()
  })
})

describe('useCatalystItemSizes', () => {
  it('measures every item and leaves out one that failed, without staying pending', async () => {
    const broken = item('broken', { 'male/hat.glb': 'bafgone', 'thumbnail.png': 'bafthumb', 'image.png': 'bafimage' })
    const { result } = renderHook(() => useCatalystItemSizes([hat, broken]), { wrapper: wrapperWith() })
    await waitFor(() => expect(result.current.isMeasuring).toBe(false))
    expect(result.current.sizes.get('hat')).toEqual({ bytes: 2_250_000, limit: 3 * 1024 * 1024 })
    expect(result.current.sizes.has('broken')).toBe(false)
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'item_size', itemId: 'broken' })
  })

  it('measures an item that failed again on request, once storage answers', async () => {
    const flaky = item('flaky', { 'male/hat.glb': 'bafflaky', 'thumbnail.png': 'bafthumb', 'image.png': 'bafimage' })
    const { result } = renderHook(() => useCatalystItemSizes([flaky]), { wrapper: wrapperWith() })
    await waitFor(() => expect(result.current.unknown.has('flaky')).toBe(true))

    STORED.bafflaky = 1_000_000
    await act(() => result.current.remeasure(flaky))
    await waitFor(() => expect(result.current.sizes.get('flaky')?.bytes).toBe(1_250_000))
    expect(result.current.unknown.has('flaky')).toBe(false)
    delete STORED.bafflaky
  })
})

describe('itemSizeDeps', () => {
  it('reads a stored file once for the session, and never one whose bytes it measured before an upload', async () => {
    const client = new QueryClient()
    const deps = itemSizeDeps(client)
    const upload = { ...hat, contents: { ...hat.contents, 'male/hat.glb': 'bafnew' } }
    await measureItemBytes({ item: upload, blobs: { 'male/hat.glb': new Blob([new Uint8Array(500)]) } }, deps)
    vi.mocked(fetchContentSize).mockClear()

    expect(await measureItemBytes({ item: upload }, itemSizeDeps(client))).toBe(250_500)
    expect(fetchContentSize).not.toHaveBeenCalled()
  })

  it('gives up on a stored file whose size never arrives, as an error rather than a pending state', async () => {
    vi.mocked(fetchContentSize).mockImplementation(
      (_hash, signal) =>
        new Promise((_resolve, reject) => signal?.addEventListener('abort', () => reject(new Error('timed out'))))
    )
    const { sizeOf } = itemSizeDeps(new QueryClient(), { timeoutMs: 20 })
    await expect(sizeOf('bafstalled')).rejects.toThrow('timed out')
  })

  it(`keeps at most ${MAX_CONCURRENT_MEASUREMENTS} stored files measuring at once`, async () => {
    let inFlight = 0
    let peak = 0
    vi.mocked(fetchContentSize).mockImplementation(async () => {
      inFlight++
      peak = Math.max(peak, inFlight)
      await new Promise(resolve => setTimeout(resolve, 5))
      inFlight--
      return 10
    })
    const { sizeOf } = itemSizeDeps(new QueryClient())
    const sizes = await Promise.all(Array.from({ length: 20 }, (_, index) => sizeOf(`baf${index}`)))
    expect(sizes).toHaveLength(20)
    expect(peak).toBe(MAX_CONCURRENT_MEASUREMENTS)
  })
})
