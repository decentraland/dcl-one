import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchMapGridCheckedAt, gridCheckedAt, mapGridAgeLabel, MAP_GRID_STALE_AFTER_MS } from '../features/map/gridAge'

const NOW = Date.UTC(2026, 10, 24, 12, 0, 0)
const MINUTE = 60_000

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('the parcel map age', () => {
  it('reads checkedAt from a tiles info answer', () => {
    expect(gridCheckedAt({ lastUpdatedAt: NOW - 600 * MINUTE, checkedAt: NOW - 7 * MINUTE })).toBe(NOW - 7 * MINUTE)
  })

  it('has no age without checkedAt, whatever lastUpdatedAt says', () => {
    expect(gridCheckedAt({ lastUpdatedAt: NOW - 600 * MINUTE })).toBeNull()
    for (const checkedAt of [0, -1, '1795000000000', null, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(gridCheckedAt({ checkedAt })).toBeNull()
    }
    expect(gridCheckedAt(null)).toBeNull()
    expect(gridCheckedAt('checkedAt')).toBeNull()
  })

  it('shows no label for a grid found current in the last five minutes', () => {
    expect(mapGridAgeLabel(NOW, NOW)).toBeNull()
    expect(mapGridAgeLabel(NOW - MAP_GRID_STALE_AFTER_MS, NOW)).toBeNull()
    // A clock behind the server's reads a grid from the future: it is current.
    expect(mapGridAgeLabel(NOW + 3 * MINUTE, NOW)).toBeNull()
    expect(mapGridAgeLabel(null, NOW)).toBeNull()
  })

  it('says how long ago an older grid was found current', () => {
    expect(mapGridAgeLabel(NOW - MAP_GRID_STALE_AFTER_MS - 1, NOW)).toBe('Updated 5 min ago')
    expect(mapGridAgeLabel(NOW - 59 * MINUTE, NOW)).toBe('Updated 59 min ago')
    expect(mapGridAgeLabel(NOW - 60 * MINUTE, NOW)).toBe('Updated 1 h ago')
    expect(mapGridAgeLabel(NOW - (23 * 60 + 59) * MINUTE, NOW)).toBe('Updated 23 h ago')
    expect(mapGridAgeLabel(NOW - 24 * 60 * MINUTE, NOW)).toBe('Updated 1 day ago')
    expect(mapGridAgeLabel(NOW - 3 * 24 * 60 * MINUTE, NOW)).toBe('Updated 3 days ago')
  })
})

describe('reading the parcel map age', () => {
  it('asks the map API for its tiles info', async () => {
    const fetch = vi.fn(async () => Response.json({ lastUpdatedAt: 1, checkedAt: NOW }))
    vi.stubGlobal('fetch', fetch)
    await expect(fetchMapGridCheckedAt('https://map.test/')).resolves.toBe(NOW)
    expect(fetch).toHaveBeenCalledWith('https://map.test/v2/tiles/info', { signal: undefined })
  })

  it('has no age when the API cannot serve its grid, and releases the answer', async () => {
    const cancel = vi.fn(async () => undefined)
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 503, body: { cancel } }) as unknown as Response))
    await expect(fetchMapGridCheckedAt('https://map.test')).resolves.toBeNull()
    expect(cancel).toHaveBeenCalledOnce()
  })

  it('fails when the request does', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('offline'))))
    await expect(fetchMapGridCheckedAt('https://map.test')).rejects.toThrow('offline')
  })
})
