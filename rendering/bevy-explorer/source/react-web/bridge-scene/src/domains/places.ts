// Places for the full-screen map: the places covering parcels, the destinations search, the
// user's like and favourite on a place, and a place's photos.
//   from: the places service via BevyApi.kernelFetch, signed so each place carries the signed-in
//         user's own like, dislike and favourite;
//       + camera-reel for a place's photos;
//       + one place by id for the lobby's place details, with its connected players;
//       + whether the player may enter a world, from the engine's entry check.
import type { DiscoverPlace, PlacePhoto, SearchPlacesRequest, SignedPlace } from '@explorer-core/engine/protocol'
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { serviceBase, signed, signedBody } from '../http'

type PlaceRaw = Partial<Omit<DiscoverPlace, 'description' | 'image' | 'contact_name' | 'world_name'>> & {
  id: string
  description?: string | null
  image?: string | null
  contact_name?: string | null
  world_name?: string | null
}

function mapPlace(p: PlaceRaw): DiscoverPlace {
  const positions = Array.isArray(p.positions) ? p.positions : []
  return {
    ...p,
    title: p.title ?? '',
    description: p.description ?? '',
    image: p.image ?? '',
    positions,
    base_position: p.base_position ?? positions[0] ?? '',
    owner: p.owner ?? null,
    contact_name: p.contact_name ?? undefined,
    world_name: p.world_name ?? undefined
  }
}

const places = (rows: PlaceRaw[] | undefined): DiscoverPlace[] => (Array.isArray(rows) ? rows.map(mapPlace) : [])

async function placesAt(positions: string[]): Promise<DiscoverPlace[]> {
  if (positions.length === 0) return []
  const query = positions.map((p) => `positions=${p}`).join('&')
  return places(await signed<PlaceRaw[]>(`${await serviceBase('places')}/api/places?${query}&with_realms_detail=true`))
}

const queryValue = (value: string): string => encodeURIComponent(value).replace(/%20/g, '+')

async function search(msg: SearchPlacesRequest): Promise<{ places: DiscoverPlace[]; total: number }> {
  const params = [
    `search=${queryValue(msg.search)}`,
    `offset=${msg.offset}`,
    `limit=${msg.limit}`,
    `order_by=${msg.orderBy}`,
    'order=desc',
    `categories=${msg.favorites ? '' : encodeURIComponent(msg.category)}`,
    'with_realms_detail=true',
    'only_sdk7=true'
  ]
  if (msg.favorites) params.push('only_favorites=true')
  // the whole body: the page needs `total` beside the places
  const body = await signedBody<{ data?: PlaceRaw[]; total?: number }>(`${await serviceBase('places')}/api/destinations?${params.join('&')}`)
  const found = places(body?.data)
  return { places: found, total: body?.total ?? found.length }
}

const placeUrl = async (id: string, what: string): Promise<string> => `${await serviceBase('places')}/api/places/${encodeURIComponent(id)}/${what}`

async function photos(placeId: string, offset: number, limit: number): Promise<{ images: PlacePhoto[]; total: number }> {
  const url = `${await serviceBase('cameraReel')}/api/places/${encodeURIComponent(placeId)}/images?limit=${limit}&offset=${offset}`
  const body = await signedBody<{ images?: PlacePhoto[]; maxImages?: number }>(url)
  const images = (body?.images ?? []).filter((i) => typeof i.url === 'string')
  return { images, total: body?.maxImages ?? images.length }
}

// Every request is answered, a failed one with `failed`, so the page never waits for nothing.
export function registerPlaces(ctx: Ctx): void {
  ctx.on('getPlace', async (msg) => {
    try {
      const url = `${await serviceBase('places')}/api/destinations?ids=${encodeURIComponent(msg.id)}&with_connected_users=true`
      const rows = await signed<SignedPlace[]>(url)
      ctx.send({ kind: 'place', requestId: msg.requestId, place: Array.isArray(rows) ? (rows.find((p) => p.id === msg.id) ?? null) : null })
    } catch (e) {
      console.error('[places] place read failed', e)
      ctx.send({ kind: 'place', requestId: msg.requestId, place: null })
    }
  })
  ctx.on('getWorldAccess', async (msg) => {
    try {
      ctx.send({ kind: 'worldAccess', requestId: msg.requestId, access: (await BevyApi.getWorldAccess?.(msg.world)) ?? 'failed' })
    } catch (e) {
      console.error('[places] world access failed', e)
      ctx.send({ kind: 'worldAccess', requestId: msg.requestId, access: 'failed' })
    }
  })
  ctx.on('getPlacesAt', async (msg) => {
    try {
      ctx.send({ kind: 'placesAt', request: msg.request, places: await placesAt(msg.positions) })
    } catch (e) {
      console.error('[places] by parcel failed', e)
      ctx.send({ kind: 'placesAt', request: msg.request, places: [], failed: true })
    }
  })
  ctx.on('searchPlaces', async (msg) => {
    try {
      ctx.send({ kind: 'placeSearch', request: msg.request, ...(await search(msg)) })
    } catch (e) {
      console.error('[places] search failed', e)
      ctx.send({ kind: 'placeSearch', request: msg.request, places: [], total: 0, failed: true })
    }
  })
  ctx.on('setPlaceLike', async (msg) => {
    try {
      const vote = await signed<{ likes: number; dislikes: number; user_like: boolean; user_dislike: boolean }>(await placeUrl(msg.placeId, 'likes'), 'PATCH', {
        like: msg.like
      })
      if (vote == null) throw new Error('the place did not take the vote')
      ctx.send({ kind: 'placeLike', request: msg.request, ...vote })
    } catch (e) {
      console.error('[places] like failed', e)
      ctx.send({ kind: 'placeLike', request: msg.request, likes: 0, dislikes: 0, user_like: false, user_dislike: false, failed: true })
    }
  })
  ctx.on('setPlaceFavorite', async (msg) => {
    try {
      const fav = await signed<{ favorites: number; user_favorite: boolean }>(await placeUrl(msg.placeId, 'favorites'), 'PATCH', { favorites: msg.favorite })
      if (fav == null) throw new Error('the place did not take the favourite')
      ctx.send({ kind: 'placeFavorite', request: msg.request, ...fav })
    } catch (e) {
      console.error('[places] favourite failed', e)
      ctx.send({ kind: 'placeFavorite', request: msg.request, favorites: 0, user_favorite: false, failed: true })
    }
  })
  ctx.on('getPlacePhotos', async (msg) => {
    try {
      ctx.send({ kind: 'placePhotos', request: msg.request, ...(await photos(msg.placeId, msg.offset, msg.limit)) })
    } catch (e) {
      console.error('[places] photos failed', e)
      ctx.send({ kind: 'placePhotos', request: msg.request, images: [], total: 0, failed: true })
    }
  })
}
