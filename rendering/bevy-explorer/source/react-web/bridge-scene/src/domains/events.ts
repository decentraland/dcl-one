// Events: the lists the Events page shows, the user's reminder on an event, and the places
// events are held at.
//   from: the events service via BevyApi.kernelFetch. The reads are signed so each event says
//         whether the signed-in user asked to be reminded (`attending`), and carry who is at
//         the event now (`with_connected_users`).
//       + the places service for an event's place (POST /api/destinations with the ids).
import { serviceBase, signed } from '../http'
import { destinationsByIds } from '../destinations'
import type { Ctx } from '../bridge'
import type { EventData, EventPlace, GetEventsRequest } from '@explorer-core/engine/protocol'

type EventRaw = Partial<EventData> & { id: string }

function mapEvent(e: EventRaw): EventData {
  return {
    id: e.id,
    name: e.name ?? '',
    description: e.description ?? null,
    image: e.image ?? null,
    image_vertical: e.image_vertical ?? null,
    start_at: e.start_at ?? null,
    finish_at: e.finish_at ?? null,
    next_start_at: e.next_start_at ?? null,
    next_finish_at: e.next_finish_at ?? null,
    duration: e.duration ?? null,
    x: e.x ?? 0,
    y: e.y ?? 0,
    server: e.server ?? null,
    world: e.world === true,
    user_name: e.user_name ?? null,
    scene_name: e.scene_name ?? null,
    highlighted: e.highlighted === true,
    live: e.live === true,
    attending: e.attending === true,
    total_attendees: e.total_attendees ?? 0,
    recurrent: e.recurrent === true,
    recurrent_dates: Array.isArray(e.recurrent_dates) ? e.recurrent_dates : [],
    connected_addresses: Array.isArray(e.connected_addresses) ? e.connected_addresses : [],
    community_id: e.community_id ?? null,
    place_id: e.place_id ?? null
  }
}

function listUrl(base: string, msg: GetEventsRequest): string {
  const params: string[] = []
  if (msg.limit != null) params.push(`limit=${msg.limit}`)
  if (msg.offset != null) params.push(`offset=${msg.offset}`)
  if (msg.list != null) params.push(`list=${msg.list}`)
  if (msg.from != null) params.push(`from=${encodeURIComponent(msg.from)}`)
  if (msg.to != null) params.push(`to=${encodeURIComponent(msg.to)}`)
  params.push('with_connected_users=true')
  return `${base}/api/events?${params.join('&')}`
}

async function list(msg: GetEventsRequest): Promise<EventData[]> {
  const rows = await signed<EventRaw[]>(listUrl(await serviceBase('events'), msg))
  if (!Array.isArray(rows)) throw new Error('events answer has no list')
  return rows.map(mapEvent)
}

async function setAttending(eventId: string, attending: boolean): Promise<void> {
  const url = `${await serviceBase('events')}/api/events/${encodeURIComponent(eventId)}/attendees`
  await signed(url, attending ? 'POST' : 'DELETE')
}

async function places(ids: string[]): Promise<EventPlace[]> {
  const rows = await destinationsByIds(`${await serviceBase('places')}/api/destinations`, ids)
  return rows.map((p) => ({ id: p.id, title: p.title ?? '', world_name: p.world_name ?? null, base_position: p.base_position ?? null }))
}

// Every request is answered, a failed one with `failed`, so the page leaves its loading state.
export function registerEvents(ctx: Ctx): void {
  ctx.on('getEvents', async (msg) => {
    try {
      ctx.send({ kind: 'events', request: msg.request, events: await list(msg) })
    } catch (e) {
      console.error('[events] list failed', e)
      ctx.send({ kind: 'events', request: msg.request, events: [], failed: true })
    }
  })
  ctx.on('setEventAttending', async (msg) => {
    try {
      await setAttending(msg.eventId, msg.attending)
      ctx.send({ kind: 'eventAttending', eventId: msg.eventId, attending: msg.attending })
    } catch (e) {
      console.error('[events] reminder change failed', e)
      ctx.send({ kind: 'eventAttending', eventId: msg.eventId, attending: msg.attending, failed: true })
    }
  })
  ctx.on('getEventPlaces', async (msg) => {
    try {
      ctx.send({ kind: 'eventPlaces', request: msg.request, places: msg.placeIds.length > 0 ? await places(msg.placeIds) : [] })
    } catch (e) {
      console.error('[events] places failed', e)
      ctx.send({ kind: 'eventPlaces', request: msg.request, places: [], failed: true })
    }
  })
}
