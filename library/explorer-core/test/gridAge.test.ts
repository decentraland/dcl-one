import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchMapGridAge, gridCheckedAt, mapGridAgeLabel, MAP_GRID_STALE_AFTER_MS } from '../features/map/gridAge'
import { fixMapPublicReads } from '../features/map/mapApi'

const NOW = Date.UTC(2026, 10, 24, 12, 0, 0)
const MINUTE = 60_000

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
  fixMapPublicReads(null)
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
    vi.useFakeTimers({ now: NOW })
    const fetch = vi.fn(async () => Response.json({ lastUpdatedAt: 1, checkedAt: NOW }))
    vi.stubGlobal('fetch', fetch)
    await expect(fetchMapGridAge('https://map.test/')).resolves.toEqual({ checkedAt: NOW, skew: 0 })
    expect(fetch).toHaveBeenCalledWith('https://map.test/v2/tiles/info', { signal: undefined })
  })

  it("measures the server's clock against this device's from the answer's Date", async () => {
    // This device runs six minutes ahead of the server.
    vi.useFakeTimers({ now: NOW + 6 * MINUTE })
    const date = new Date(NOW).toUTCString()
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ checkedAt: NOW - MINUTE }, { headers: { date } })))
    const age = await fetchMapGridAge('https://map.test')
    expect(age).toEqual({ checkedAt: NOW - MINUTE, skew: -6 * MINUTE })
    expect(mapGridAgeLabel(age!.checkedAt, Date.now() + age!.skew)).toBeNull()
    expect(mapGridAgeLabel(age!.checkedAt, Date.now()), 'on the device clock alone').toBe('Updated 7 min ago')
  })

  it("measures it on the server's clock as well when this device runs behind", async () => {
    // This device runs ten minutes behind the server, whose grid was checked twelve minutes ago.
    vi.useFakeTimers({ now: NOW - 10 * MINUTE })
    const date = new Date(NOW).toUTCString()
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ checkedAt: NOW - 12 * MINUTE }, { headers: { date } })))
    const age = await fetchMapGridAge('https://map.test')
    expect(age).toEqual({ checkedAt: NOW - 12 * MINUTE, skew: 10 * MINUTE })
    expect(mapGridAgeLabel(age!.checkedAt, Date.now() + age!.skew)).toBe('Updated 12 min ago')
    expect(mapGridAgeLabel(age!.checkedAt, Date.now()), 'on the device clock alone').toBeNull()
  })

  it('has no age when the answer has no checkedAt', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ lastUpdatedAt: NOW - 600 * MINUTE })))
    await expect(fetchMapGridAge('https://map.test')).resolves.toBeNull()
  })

  it('has no age when the map API says it cannot serve its grid (its unavailable envelope)', async () => {
    const envelope = { error: 'Service Unavailable', message: 'the tile grid has not been built yet', source: 'map-grid' }
    vi.stubGlobal('fetch', vi.fn(async () => Response.json(envelope, { status: 503 })))
    await expect(fetchMapGridAge('https://map.test')).resolves.toBeNull()
  })

  it('fails on any other error answer, releasing it, so a caller keeps its last answer', async () => {
    for (const status of [502, 429, 503]) {
      const cancel = vi.fn(async () => undefined)
      const json = vi.fn(async () => ({ message: 'Bad Gateway' }))
      vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status, json, body: { cancel } }) as unknown as Response))
      await expect(fetchMapGridAge('https://map.test'), `${status}`).rejects.toThrow(`answered ${status}`)
      expect(cancel, `${status}`).toHaveBeenCalledOnce()
    }
  })

  it("takes the server's clock from the answer's serverNow, with no readable Date", async () => {
    // A cross-origin answer whose Date this device cannot read; the device runs six minutes ahead.
    vi.useFakeTimers({ now: NOW + 6 * MINUTE })
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ checkedAt: NOW - MINUTE, serverNow: NOW })))
    const age = await fetchMapGridAge('https://map.test')
    expect(age).toEqual({ checkedAt: NOW - MINUTE, skew: -6 * MINUTE })
    expect(mapGridAgeLabel(age!.checkedAt, Date.now() + age!.skew)).toBeNull()
  })

  it("prefers serverNow to Date, and measures on this device's clock with neither", async () => {
    vi.useFakeTimers({ now: NOW })
    const date = new Date(NOW - 10 * MINUTE).toUTCString()
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ checkedAt: NOW, serverNow: NOW + 2_000 }, { headers: { date } })))
    await expect(fetchMapGridAge('https://map.test')).resolves.toEqual({ checkedAt: NOW, skew: 2_000 })
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ checkedAt: NOW })))
    await expect(fetchMapGridAge('https://map.test')).resolves.toEqual({ checkedAt: NOW, skew: 0 })
  })

  it('fails when the request does', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('offline'))))
    await expect(fetchMapGridAge('https://map.test')).rejects.toThrow('offline')
  })

  it("answers the mock mode's fixed age and reads nothing", async () => {
    const fetch = vi.fn()
    vi.stubGlobal('fetch', fetch)
    fixMapPublicReads({ pois: [], peers: [] })
    await expect(fetchMapGridAge('https://map.test')).resolves.toBeNull()
    fixMapPublicReads({ pois: [], peers: [], gridAge: { checkedAt: NOW, skew: 0 } })
    await expect(fetchMapGridAge('https://map.test')).resolves.toEqual({ checkedAt: NOW, skew: 0 })
    expect(fetch).not.toHaveBeenCalled()
  })
})
