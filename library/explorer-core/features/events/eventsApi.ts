// The public events lists the lobby and the sidebar's live counter read (<events service>/api/events).
// The Events page reads through the bridge instead: see the HUD's useEventsData.

import { serviceUrl } from '@hud-host'
import type { EventData } from '../../engine/protocol'

export const EVENTS_API = `${serviceUrl('events')}/api/events`
// Unity refreshes the sidebar's live counter every 3 minutes (SidebarController.FillLiveEventsAsync).
export const LIVE_COUNT_REFRESH_MS = 3 * 60 * 1000

export type EventsList = 'live' | 'upcoming'

/** An event from the public, unsigned list; the service may leave out the optional fields. */
export type DclEvent = Pick<EventData, 'id' | 'name' | 'x' | 'y'> & { start_at: string } & Partial<
    Pick<
      EventData,
      | 'description'
      | 'image'
      | 'server'
      | 'world'
      | 'live'
      | 'finish_at'
      | 'next_start_at'
      | 'next_finish_at'
      | 'duration'
      | 'highlighted'
      | 'total_attendees'
      | 'user_name'
      | 'connected_addresses'
    >
  >

/** `connected` asks for who is at each event now (`connected_addresses`); without it the service sends none. */
export async function fetchEvents(list: EventsList, { signal, connected = false }: { signal?: AbortSignal; connected?: boolean } = {}): Promise<DclEvent[]> {
  const res = await fetch(`${EVENTS_API}?list=${list}${connected ? '&with_connected_users=true' : ''}`, { signal })
  if (!res.ok) {
    void res.body?.cancel()
    throw new Error(`Events service returned ${res.status}`)
  }
  const body = (await res.json()) as { ok?: boolean; data?: DclEvent[] }
  if (body.ok === false || !Array.isArray(body.data)) throw new Error('Events service returned an unexpected response')
  return body.data
}

export type EventDestination = { kind: 'world'; realm: string } | { kind: 'parcel'; x: number; y: number }

/** A world event's `server` must be a world's name: the events service takes any text there, and
 *  the engine reads anything but a name as a realm's address. */
export function eventDestination(e: Pick<DclEvent, 'world' | 'server' | 'x' | 'y'>): EventDestination | null {
  if (e.world) return e.server != null && /^[a-z0-9-]+(\.dcl)?\.eth$/i.test(e.server) ? { kind: 'world', realm: e.server } : null
  return Number.isFinite(e.x) && Number.isFinite(e.y) ? { kind: 'parcel', x: e.x, y: e.y } : null
}

export function eventLocation(e: DclEvent): string {
  return e.world ? (e.server ?? '') : `${e.x},${e.y}`
}

/** When the event starts next. `start_at` is the first date of a series; the lists select and order on the next one. */
export function eventStartAt(e: Pick<DclEvent, 'start_at' | 'next_start_at'>): string {
  return e.next_start_at ?? e.start_at
}

export function eventWhen(e: DclEvent): string {
  if (e.live) return 'Happening now'
  return new Date(eventStartAt(e)).toLocaleString(undefined, { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })
}
