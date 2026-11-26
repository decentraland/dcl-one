import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage, SearchPlacesRequest } from '@explorer-core/engine/protocol'

const { signed, signedBody } = vi.hoisted(() => ({ signed: vi.fn(), signedBody: vi.fn() }))
vi.mock('../../bridge-scene/src/http', () => ({ serviceBase: async (name: string) => `https://${name}.example.org`, signed, signedBody }))

const { registerPlaces } = await import('../../bridge-scene/src/domains/places')

function fakeCtx(): { sent: SceneToPage[]; req: (msg: PageToScene) => Promise<void> } {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Array<(m: PageToScene) => unknown>>()
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, [...(handlers.get(kind) ?? []), h as (m: PageToScene) => unknown]),
    push: () => {}
  }
  registerPlaces(ctx)
  return {
    sent,
    req: async (msg) => {
      await Promise.all((handlers.get(msg.kind) ?? []).map((h) => h(msg)))
    }
  }
}

const search = (over: Partial<SearchPlacesRequest> = {}): SearchPlacesRequest => ({
  kind: 'searchPlaces',
  request: 1,
  search: '',
  category: '',
  favorites: false,
  orderBy: 'most_active',
  offset: 0,
  limit: 50,
  ...over
})

beforeEach(() => {
  signed.mockReset()
  signedBody.mockReset()
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('bridge places', () => {
  it('reads the places covering parcels, and fills what the service leaves out', async () => {
    const h = fakeCtx()
    signed.mockResolvedValueOnce([{ id: 'a', title: 'Plaza', positions: ['0,0', '1,0'], description: null, image: null, user_like: true }])
    await h.req({ kind: 'getPlacesAt', request: 4, positions: ['0,0', '1,0'] })
    expect(signed).toHaveBeenCalledWith('https://places.example.org/api/places?positions=0,0&positions=1,0&with_realms_detail=true')
    expect(h.sent).toEqual([
      {
        kind: 'placesAt',
        request: 4,
        places: [{ id: 'a', title: 'Plaza', positions: ['0,0', '1,0'], base_position: '0,0', description: '', image: '', owner: null, user_like: true, contact_name: undefined, world_name: undefined }]
      }
    ])
  })

  it('builds the destinations query the services expect, and keeps the total', async () => {
    const h = fakeCtx()
    signedBody.mockResolvedValue({ data: [], total: 120 })
    await h.req(search({ search: 'sugar club', category: 'music', orderBy: 'like_score', offset: 100 }))
    await h.req(search({ favorites: true, category: 'music' }))
    // what the player typed stays one value
    await h.req(search({ search: 'a&only_favorites=true #b%c+d' }))
    const [first, favourites, typed] = signedBody.mock.calls.map((c) => c[0] as string)
    expect(first).toBe(
      'https://places.example.org/api/destinations?search=sugar+club&offset=100&limit=50&order_by=like_score&order=desc&categories=music&with_realms_detail=true&only_sdk7=true'
    )
    expect(favourites).toBe('https://places.example.org/api/destinations?offset=0&limit=50&order_by=most_active&order=desc&with_realms_detail=true&only_sdk7=true&only_favorites=true')
    const query = new URLSearchParams(typed.split('?')[1])
    expect(query.get('search')).toBe('a&only_favorites=true #b%c+d')
    expect(query.has('only_favorites')).toBe(false)
    expect(h.sent[0]).toEqual({ kind: 'placeSearch', request: 1, places: [], total: 120 })
  })

  it('leaves an empty search and an empty category out of the query, which the service answers with no places', async () => {
    const h = fakeCtx()
    signedBody.mockResolvedValue({ data: [], total: 0 })
    await h.req(search())
    await h.req(search({ search: 'plaza' }))
    await h.req(search({ category: 'art' }))
    const [all, typed, category] = signedBody.mock.calls.map((c) => (c[0] as string).split('?')[1])
    expect(all).toBe('offset=0&limit=50&order_by=most_active&order=desc&with_realms_detail=true&only_sdk7=true')
    expect(typed).toBe('search=plaza&offset=0&limit=50&order_by=most_active&order=desc&with_realms_detail=true&only_sdk7=true')
    expect(category).toBe('offset=0&limit=50&order_by=most_active&order=desc&categories=art&with_realms_detail=true&only_sdk7=true')
  })

  it('votes and favourites with a PATCH, and reports a refusal as failed', async () => {
    const h = fakeCtx()
    signed.mockResolvedValueOnce({ likes: 4, dislikes: 1, user_like: true, user_dislike: false }).mockResolvedValueOnce({ favorites: 2, user_favorite: true })
    await h.req({ kind: 'setPlaceLike', request: 2, placeId: 'a b', like: true })
    await h.req({ kind: 'setPlaceFavorite', request: 3, placeId: 'a b', favorite: true })
    expect(signed.mock.calls).toEqual([
      ['https://places.example.org/api/places/a%20b/likes', 'PATCH', { like: true }],
      ['https://places.example.org/api/places/a%20b/favorites', 'PATCH', { favorites: true }]
    ])
    expect(h.sent).toEqual([
      { kind: 'placeLike', request: 2, likes: 4, dislikes: 1, user_like: true, user_dislike: false },
      { kind: 'placeFavorite', request: 3, favorites: 2, user_favorite: true }
    ])
    signed.mockRejectedValueOnce(new Error('HTTP 401'))
    await h.req({ kind: 'setPlaceLike', request: 5, placeId: 'a', like: null })
    expect(h.sent.at(-1)).toMatchObject({ kind: 'placeLike', request: 5, failed: true })
  })

  it("reads a page of a place's photos with their count", async () => {
    const h = fakeCtx()
    signedBody.mockResolvedValueOnce({ images: [{ id: 'p', url: 'https://x/p.jpg' }, { id: 'broken' }], maxImages: 7 })
    await h.req({ kind: 'getPlacePhotos', request: 6, placeId: 'plaza', offset: 100, limit: 100 })
    expect(signedBody).toHaveBeenCalledWith('https://cameraReel.example.org/api/places/plaza/images?limit=100&offset=100')
    expect(h.sent).toEqual([{ kind: 'placePhotos', request: 6, images: [{ id: 'p', url: 'https://x/p.jpg' }], total: 7 }])
  })
})
