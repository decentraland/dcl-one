import { afterEach, describe, it, expect, vi } from 'vitest'
import { BASE_PX_PER_PARCEL, type ParcelRect, type SatelliteManifest } from '../features/map/atlas'
import { SatelliteDetail, base64ToBlob, type DetailView } from '../features/map/satelliteTiles'

const around = (x: number, y: number, half: number): ParcelRect => ({ xMin: x - half, xMax: x + half, yMin: y - half, yMax: y + half })
const view = (rect: ParcelRect, doublings: number): DetailView => ({ rect, pxPerParcel: BASE_PX_PER_PARCEL * 2 ** doublings })

// every base chunk has tiles
const FULL: SatelliteManifest = new Map(Array.from({ length: 64 }, (_, n) => [`${n % 8},${Math.floor(n / 8)}`, 'v1']))

interface Pending {
  path: string
  resolve: (data: string | null) => void
}

function setup(options: { capacity?: number; manifest?: SatelliteManifest | null; now?: () => number } = {}) {
  const requests: Pending[] = []
  const fetchTile = vi.fn(
    (path: string) =>
      new Promise<string | null>((resolve) => {
        requests.push({ path, resolve })
      })
  )
  let urls = 0
  const createImage = vi.fn(async () => `blob:tile-${++urls}`)
  const revokeUrl = vi.fn()
  const store = new SatelliteDetail({
    fetchTile,
    loadManifest: async () => (options.manifest === undefined ? FULL : options.manifest),
    createImage,
    revokeUrl,
    capacity: options.capacity,
    now: options.now
  })
  return { store, requests, fetchTile, revokeUrl }
}

/** Lets the replies the store is waiting on run. */
const settle = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await Promise.resolve()
}

describe('base64ToBlob', () => {
  it('carries the decoded bytes as a jpeg', () => {
    const blob = base64ToBlob(btoa('abc'))
    expect(blob.type).toBe('image/jpeg')
    expect(blob.size).toBe(3)
  })
})

describe('SatelliteDetail', () => {
  it('asks for nothing at the base level or without a view', async () => {
    const { store, fetchTile } = setup()
    store.setView(view(around(0, 0, 2), 0))
    store.setView(null)
    await settle()
    expect(fetchTile).not.toHaveBeenCalled()
    expect(store.getSnapshot()).toEqual([])
  })

  it('keeps at most four requests in flight and sends the tile nearest the centre first', async () => {
    const { store, requests } = setup()
    store.setView(view(around(0, 0, 2), 5))
    await settle()
    expect(requests).toHaveLength(4)
    // the tile holding the centre parcel is among the first four: level 8, i=122, j=121 (section 3,3)
    expect(requests.some((r) => r.path.endsWith('/8/122%2C121.ktx2') || r.path.endsWith('/8/121%2C121.ktx2') || r.path.endsWith('/8/122%2C122.ktx2') || r.path.endsWith('/8/121%2C122.ktx2'))).toBe(true)
    expect(requests.every((r) => r.path.startsWith('sections/'))).toBe(true)

    requests[0].resolve('AAAA')
    await settle()
    expect(requests).toHaveLength(5)
  })

  it('does not ask for tiles of a section the manifest lacks', async () => {
    const { store, fetchTile } = setup({ manifest: new Map([['0,0', 'v1']]) })
    store.setView(view(around(0, 0, 2), 5))
    await settle()
    expect(fetchTile).not.toHaveBeenCalled()
  })

  it('asks for nothing when the source has no manifest', async () => {
    const { store, fetchTile } = setup({ manifest: null })
    store.setView(view(around(0, 0, 2), 5))
    await settle()
    expect(fetchTile).not.toHaveBeenCalled()
  })

  it('drops requests no longer wanted before sending them', async () => {
    const { store, requests } = setup()
    store.setView(view(around(0, 0, 2), 5))
    await settle()
    expect(requests).toHaveLength(4)

    store.setView(view(around(0, 0, 2), 0))
    for (const r of [...requests]) r.resolve('AAAA')
    await settle()
    expect(requests).toHaveLength(4)
  })

  it('publishes a tile once it is decoded', async () => {
    const { store, requests } = setup()
    const heard = vi.fn()
    store.subscribe(heard)
    store.setView(view(around(0, 0, 0.5), 5))
    await settle()
    requests[0].resolve('AAAA')
    await settle()
    const [tile] = store.getSnapshot()
    expect(tile.url).toMatch(/^blob:tile-/)
    expect(tile.level).toBe(8)
    expect(heard).toHaveBeenCalled()
  })

  it('keeps a coarser loaded tile in place until the finer one arrives', async () => {
    const { store, requests } = setup()
    store.setView(view(around(0, 0, 0.5), 2))
    await settle()
    expect(requests.every((r) => r.path.includes('/5/'))).toBe(true)
    for (const r of [...requests]) r.resolve('AAAA')
    await settle()
    const coarse = store.getSnapshot()
    expect(coarse.length).toBeGreaterThan(0)
    expect(coarse.every((t) => t.level === 5)).toBe(true)

    store.setView(view(around(0, 0, 0.5), 5))
    await settle()
    // level-8 tiles are on their way; the level-5 tile still covers them
    expect(store.getSnapshot().map((t) => t.key)).toEqual(coarse.map((t) => t.key))

    const fine = requests.filter((r) => r.path.includes('/8/'))
    expect(fine.length).toBeGreaterThan(0)
    for (const r of fine) r.resolve('AAAA')
    await settle()
    const levels = store.getSnapshot().map((t) => t.level)
    expect(levels).toContain(8)
    // coarse first, so the fine tiles draw on top
    expect(levels).toEqual([...levels].sort((a, b) => a - b))
  })

  it('revokes the least recently drawn tiles beyond its capacity', async () => {
    const { store, requests, revokeUrl } = setup({ capacity: 1 })
    store.setView(view(around(-100, 100, 0.5), 5))
    await settle()
    for (const r of [...requests]) r.resolve('AAAA')
    await settle()
    const first = store.getSnapshot().map((t) => t.url)
    expect(first.length).toBeGreaterThan(0)

    store.setView(view(around(100, -100, 0.5), 5))
    await settle()
    requests[requests.length - 1].resolve('AAAA')
    await settle()
    expect(revokeUrl).toHaveBeenCalled()
    expect(revokeUrl.mock.calls.map(([u]) => u)).toEqual(first)
  })

  it('does not ask again for a failed tile until the wait is over', async () => {
    let clock = 0
    const { store, requests, fetchTile } = setup({ now: () => clock })
    store.setView(view(around(0, 0, 0.5), 5))
    await settle()
    const asked = fetchTile.mock.calls.length
    for (const r of [...requests]) r.resolve(null)
    await settle()
    expect(store.getSnapshot()).toEqual([])
    expect(fetchTile.mock.calls.length).toBe(asked)

    clock = 31_000
    store.setView(view(around(0, 0, 0.5), 5))
    await settle()
    expect(fetchTile.mock.calls.length).toBeGreaterThan(asked)
  })

  it('revokes every tile on dispose and ignores later replies', async () => {
    const { store, requests, revokeUrl } = setup()
    store.setView(view(around(0, 0, 0.5), 5))
    await settle()
    requests[0].resolve('AAAA')
    await settle()
    const urls = store.getSnapshot().map((t) => t.url)
    store.dispose()
    expect(revokeUrl.mock.calls.map(([u]) => u)).toEqual(expect.arrayContaining(urls))
    for (const r of requests.slice(1)) r.resolve('AAAA')
    await settle()
    expect(store.getSnapshot().length).toBe(urls.length)
  })

  describe('after a failure, with the view kept still', () => {
    afterEach(() => {
      vi.useRealTimers()
    })

    it('asks for a failed tile again once the wait is over', async () => {
      vi.useFakeTimers()
      const { store, requests, fetchTile } = setup()
      store.setView(view(around(0, 0, 0.5), 5))
      await settle()
      const failedPath = requests[0].path
      requests[0].resolve(null)
      await settle()
      expect(fetchTile.mock.calls.filter(([p]) => p === failedPath)).toHaveLength(1)

      await vi.advanceTimersByTimeAsync(30_000)
      await settle()
      expect(fetchTile.mock.calls.filter(([p]) => p === failedPath)).toHaveLength(2)
      store.dispose()
    })

    it('loads the manifest again after a failure, and gives up after repeated ones', async () => {
      vi.useFakeTimers()
      const loadManifest = vi.fn(async (): Promise<SatelliteManifest | null> => {
        throw new Error('offline')
      })
      const fetchTile = vi.fn(async () => null)
      const store = new SatelliteDetail({ fetchTile, loadManifest, createImage: async () => 'blob:x', revokeUrl: vi.fn() })
      store.setView(view(around(0, 0, 0.5), 5))
      await settle()
      expect(loadManifest).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(30_000)
      await settle()
      expect(loadManifest).toHaveBeenCalledTimes(2)

      // eight failures in a row with nothing loaded: no more attempts, and no retry left pending
      for (let i = 0; i < 10; i++) {
        await vi.advanceTimersByTimeAsync(30_000)
        await settle()
      }
      expect(loadManifest).toHaveBeenCalledTimes(8)
      expect(vi.getTimerCount()).toBe(0)
      expect(fetchTile).not.toHaveBeenCalled()
    })

    it('leaves no retry pending once disposed', async () => {
      vi.useFakeTimers()
      const { store, requests } = setup()
      store.setView(view(around(0, 0, 0.5), 5))
      await settle()
      requests[0].resolve(null)
      await settle()
      expect(vi.getTimerCount()).toBe(1)
      store.dispose()
      expect(vi.getTimerCount()).toBe(0)
    })
  })
})
