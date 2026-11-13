// The links the map hands out: a jump-in link to a parcel, an event's page, a post on X, and
// an entry in the player's calendar.

import { siteUrl } from '@hud-host'
import type { MapEvent, MapPlace } from './mapApi'
import { parseParcel } from './mapCoords'

/** A link that opens Decentraland at a parcel; carries the sharer's address when they have one. */
export function jumpInLink(x: number, y: number, referrer: string | undefined): string {
  const link = siteUrl(`/jump/?position=${x},${y}`)
  return referrer != null && referrer !== '' ? `${link}&referrer=${referrer}` : link
}

const X_POST = 'https://twitter.com/intent/tweet'
const HASHTAG = 'DCLPlace'

function postOnX(text: string, link: string): string {
  return `${X_POST}?text=${encodeURIComponent(text)}&hashtags=${HASHTAG}&url=${encodeURIComponent(link)}`
}

export interface ShareLinks {
  copy: string
  x: string
}

export function placeShareLinks(place: MapPlace, referrer: string | undefined): ShareLinks {
  const base = parseParcel(place.base_position) ?? { x: 0, y: 0 }
  const copy = jumpInLink(base.x, base.y, referrer)
  return { copy, x: postOnX(`Check out ${place.title}, a cool place I found in Decentraland!`, copy) }
}

/** A live event shares its place; one yet to start shares its own page. */
export function eventShareLinks(event: MapEvent, referrer: string | undefined): ShareLinks {
  const copy = event.live === true ? jumpInLink(event.x, event.y, referrer) : siteUrl(`/whats-on/?id=${encodeURIComponent(event.id)}`)
  return { copy, x: postOnX(event.name, copy) }
}

const calendarStamp = (date: Date): string => date.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')

/** A Google Calendar entry for one occurrence of an event, as long as its next occurrence runs. */
export function calendarLink(event: MapEvent, startAt: Date, referrer: string | undefined): string {
  const next = Date.parse(event.next_start_at ?? event.start_at)
  const finish = Date.parse(event.next_finish_at ?? event.finish_at ?? event.start_at)
  const duration = Number.isFinite(next) && Number.isFinite(finish) ? Math.max(0, finish - next) : 0
  const endAt = new Date(startAt.getTime() + duration)
  const details = `jump in: ${jumpInLink(event.x, event.y, referrer)}`
  return (
    'https://www.google.com/calendar/event?action=TEMPLATE' +
    `&text=${encodeURIComponent(event.name)}` +
    `&dates=${calendarStamp(startAt)}/${calendarStamp(endAt)}` +
    `&details=${encodeURIComponent(details)}`
  )
}

/** "Event started 3 hour ago" for a live event, its start as an RFC 1123 date otherwise. */
export function eventSchedule(event: MapEvent, now: number = Date.now()): string {
  const start = Date.parse(event.start_at)
  if (!Number.isFinite(start)) return ''
  if (event.live !== true) return new Date(start).toUTCString()
  const elapsed = now - start
  const days = elapsed / 86_400_000
  const hours = elapsed / 3_600_000
  if (days >= 1) return `Event started ${Math.trunc(days)} day ago`
  if (hours >= 1) return `Event started ${Math.trunc(hours)} hour ago`
  return `Event started ${Math.trunc(elapsed / 60_000)} min ago`
}

/** The dates an event repeats on, from its next occurrence on. */
export function upcomingDates(event: MapEvent): Date[] {
  const next = Date.parse(event.next_start_at ?? '')
  const out: Date[] = []
  for (const raw of event.recurrent_dates ?? []) {
    const time = Date.parse(raw)
    if (!Number.isFinite(time)) continue
    if (Number.isFinite(next) && time < next) continue
    out.push(new Date(time))
  }
  return out
}
