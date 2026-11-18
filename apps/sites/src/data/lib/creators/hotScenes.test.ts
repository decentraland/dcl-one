import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchHotScenes, placeUrl, placesUrl, selectLiveScenes, type HotScene } from './hotScenes'

const aScene = (overrides: Partial<HotScene>): HotScene => ({
  id: 'scene-1',
  name: 'A Scene',
  baseCoords: [10, 20],
  usersTotalCount: 5,
  parcels: [[10, 20]],
  thumbnail: 'https://img.example.com/scene.png',
  ...overrides
})

const okResponse = (payload: unknown) => ({ ok: true, status: 200, json: () => Promise.resolve(payload) }) as Response

afterEach(() => vi.unstubAllGlobals())

describe('fetchHotScenes', () => {
  it('reads the configured feed', async () => {
    const scenes = [aScene({})]
    const fetchMock = vi.fn().mockResolvedValue(okResponse(scenes))
    vi.stubGlobal('fetch', fetchMock)

    await expect(fetchHotScenes()).resolves.toEqual(scenes)
    expect(fetchMock).toHaveBeenCalledWith('https://realm-provider-ea.decentraland.org/hot-scenes', expect.anything())
  })

  it('drops malformed entries so a bad feed never renders broken cards', async () => {
    const good = aScene({})
    const payload = [good, { id: 'no-coords', name: 'X', usersTotalCount: 1 }, { baseCoords: [1, 2] }, null]
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(okResponse(payload)))
    await expect(fetchHotScenes()).resolves.toEqual([good])
  })

  it('treats a payload that is not a list as no scenes', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(okResponse({ error: 'shape' })))
    await expect(fetchHotScenes()).resolves.toEqual([])
  })

  it('fails on a non-ok response so react-query can retry and report it', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500, body: null }))
    await expect(fetchHotScenes()).rejects.toThrow('500')
  })
})

describe('selectLiveScenes', () => {
  it('keeps occupied scenes, busiest first, without Genesis Plaza', () => {
    const scenes = [
      aScene({ id: 'quiet', usersTotalCount: 2 }),
      aScene({ id: 'plaza', name: 'Genesis Plaza', usersTotalCount: 80 }),
      aScene({ id: 'empty', usersTotalCount: 0 }),
      aScene({ id: 'busy', usersTotalCount: 40 })
    ]
    expect(selectLiveScenes(scenes).map(scene => scene.id)).toEqual(['busy', 'quiet'])
  })

  it('caps the rail at six scenes', () => {
    const scenes = Array.from({ length: 9 }, (_, i) => aScene({ id: `s${i}`, usersTotalCount: i + 1 }))
    expect(selectLiveScenes(scenes)).toHaveLength(6)
    expect(selectLiveScenes(scenes)[0].id).toBe('s8')
  })
})

describe('place links', () => {
  it('point at the scene and the places index on the sites domain', () => {
    expect(placeUrl(aScene({ baseCoords: [-12, 7] }))).toBe('https://decentraland.zone/places/place/-12,7')
    expect(placesUrl()).toBe('https://decentraland.zone/places')
  })
})
