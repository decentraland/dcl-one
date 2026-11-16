// The public events lists the lobby and the sidebar's live counter read (<events service>/api/events).
// The Events page reads through the bridge instead: see the HUD's useEventsData.

import { serviceUrl } from '@hud-host'
import type { EventData } from '../../engine/protocol'

export const EVENTS_API = `${serviceUrl('events')}/api/events`
// Unity refreshes the sidebar's live counter every 3 minutes (SidebarController.FillLiveEventsAsync).
export const LIVE_COUNT_REFRESH_MS = 3 * 60 * 1000

export type EventsList = 'live' | 'upcoming'

/** An event from the public, unsigned list; the service may leave out the optional fields. */
export type DclEvent = Pick<EventData, 'id' | 'name' | 'x' | 'y'> &
  Partial<Pick<EventData, 'image' | 'server' | 'world' | 'live' | 'user_name' | 'connected_addresses'>>

export async function fetchEvents(list: EventsList, signal?: AbortSignal): Promise<DclEvent[]> {
  const res = await fetch(`${EVENTS_API}?list=${list}`, { signal })
  if (!res.ok) throw new Error(`Events service returned ${res.status}`)
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
