// The public events lists the lobby and the sidebar's live counter read (<events service>/api/events).
// The Events page reads through the bridge instead: see useEventsData.

import { useEffect, useState } from 'react'
import { serviceUrl } from '../../lib/baseDomain'

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

/** Live events right now; 0 while loading or when the service is unreachable (as Unity). */
export function useLiveEventCount(): number {
  const [count, setCount] = useState(0)
  useEffect(() => {
    const ctrl = new AbortController()
    const load = (): void => {
      fetchEvents('live', ctrl.signal).then(
        (list) => setCount(list.length),
        () => !ctrl.signal.aborted && setCount(0)
      )
    }
    load()
    const id = setInterval(load, LIVE_COUNT_REFRESH_MS)
    return () => {
      ctrl.abort()
      clearInterval(id)
    }
  }, [])
  return count
}
