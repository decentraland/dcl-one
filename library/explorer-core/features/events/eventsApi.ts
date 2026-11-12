// The public events lists the lobby and the sidebar's live counter read (<events service>/api/events).
// The Events page reads through the bridge instead: see the HUD's useEventsData.

import { serviceUrl } from '@hud-host'

export const EVENTS_API = `${serviceUrl('events')}/api/events`
// Unity refreshes the sidebar's live counter every 3 minutes (SidebarController.FillLiveEventsAsync).
export const LIVE_COUNT_REFRESH_MS = 3 * 60 * 1000

export type EventsList = 'live' | 'upcoming'

export interface DclEvent {
  id: string
  name: string
  image?: string | null
  x: number
  y: number
  server?: string | null
  world?: boolean
  live?: boolean
  start_at: string
  finish_at?: string
  total_attendees?: number
  user_name?: string | null
}

export async function fetchEvents(list: EventsList, signal?: AbortSignal): Promise<DclEvent[]> {
  const res = await fetch(`${EVENTS_API}?list=${list}`, { signal })
  if (!res.ok) throw new Error(`Events service returned ${res.status}`)
  const body = (await res.json()) as { ok?: boolean; data?: DclEvent[] }
  if (body.ok === false || !Array.isArray(body.data)) throw new Error('Events service returned an unexpected response')
  return body.data
}

export type EventDestination = { kind: 'world'; realm: string } | { kind: 'parcel'; x: number; y: number }

export function eventDestination(e: DclEvent): EventDestination | null {
  if (e.world) return e.server ? { kind: 'world', realm: e.server } : null
  return Number.isFinite(e.x) && Number.isFinite(e.y) ? { kind: 'parcel', x: e.x, y: e.y } : null
}
