// The full-screen map's signed requests.
//   from: places          GET   /api/places?positions=…        the places covering parcels
//                         GET   /api/destinations?…            search and category results
//                         PATCH /api/places/:id/likes          like, dislike or clear
//                         PATCH /api/places/:id/favorites
//         events          GET   /api/events?…                  live events, a place's events
//                         POST | DELETE /api/events/:id/attendees
//         camera-reel     GET   /api/places/:id/images         a place's photos
// Each goes through BevyApi.kernelFetch, so the engine signs it as the player and the services
// answer with that player's likes, favourites and attendance.
import type { MapApiRequest } from '@explorer-core/engine/protocol'
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { HttpError, serviceBase } from '../http'

const META = JSON.stringify({})

async function call(url: string, method: 'GET' | 'POST' | 'PATCH' | 'DELETE' = 'GET', body?: object): Promise<unknown> {
  const result = await BevyApi.kernelFetch({
    url,
    init: {
      headers: { 'Content-Type': 'application/json' },
      method,
      ...(body != null ? { body: JSON.stringify(body) } : {})
    },
    meta: META
  })
  if (!result.ok) throw new HttpError(result.status, result.body, result.statusText)
  return result.body ? (JSON.parse(result.body) as unknown) : undefined
}

async function run(request: MapApiRequest): Promise<unknown> {
  switch (request.op) {
    case 'places': {
      const positions = request.positions.map((p) => `positions=${p}`).join('&')
      return call(`${await serviceBase('places')}/api/places?${positions}&with_realms_detail=true`)
    }
    case 'destinations':
      return call(`${await serviceBase('places')}/api/destinations?${request.query}`)
    case 'events':
      return call(`${await serviceBase('events')}/api/events?${request.query}`)
    case 'placeLike':
      return call(`${await serviceBase('places')}/api/places/${request.id}/likes`, 'PATCH', { like: request.like })
    case 'placeFavorite':
      return call(`${await serviceBase('places')}/api/places/${request.id}/favorites`, 'PATCH', { favorites: request.favorite })
    case 'eventAttend':
      return call(`${await serviceBase('events')}/api/events/${request.id}/attendees`, request.attending ? 'POST' : 'DELETE')
    case 'placePhotos':
      return call(`${await serviceBase('cameraReel')}/api/places/${request.id}/images?limit=${request.limit}&offset=${request.offset}`)
  }
}

export function registerMapApi(ctx: Ctx): void {
  ctx.on('mapApi', async (msg) => {
    try {
      ctx.send({ kind: 'mapApiResult', id: msg.id, ok: true, data: await run(msg.request) })
    } catch (e) {
      ctx.send({ kind: 'mapApiResult', id: msg.id, ok: false, error: e instanceof Error ? e.message : String(e) })
    }
  })
}
