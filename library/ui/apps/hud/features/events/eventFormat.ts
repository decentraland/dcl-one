// What the Events page shows of an event and where its links lead: day and time labels, the
// range a calendar reads, the card an event gets, and its share, calendar and jump links.

import type { EventData, EventPlace } from '@explorer-core/engine/protocol'
import { BASE_DOMAIN } from '@hud-host'
import { withUtm } from '@explorer-core/lib/utm'

const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

const two = (n: number): string => String(n).padStart(2, '0')

/** Local midnight of the day `date` falls on. */
export function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

/** Local midnight `days` calendar days from `day` (a day is not always 24 hours). */
export function addDays(day: Date, days: number): Date {
  return new Date(day.getFullYear(), day.getMonth(), day.getDate() + days)
}

export function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

/** `yyyy-MM-dd` of a local day, for analytics. */
export function dayKey(day: Date): string {
  return `${day.getFullYear()}-${two(day.getMonth() + 1)}-${two(day.getDate())}`
}

/** "Tue, Sep 08". */
function shortDay(date: Date): string {
  return `${WEEKDAYS[date.getDay()].slice(0, 3)}, ${MONTHS[date.getMonth()]} ${two(date.getDate())}`
}

/** "4:05PM". */
function clock(date: Date): string {
  const h = date.getHours()
  return `${h % 12 === 0 ? 12 : h % 12}:${two(date.getMinutes())}${h < 12 ? 'AM' : 'PM'}`
}

/** "04:05pm". */
function paddedClock(date: Date): string {
  const h = date.getHours()
  return `${two(h % 12 === 0 ? 12 : h % 12)}:${two(date.getMinutes())}${h < 12 ? 'am' : 'pm'}`
}

/** A day's name on the calendar: "Today", "Tomorrow", else "Tue, Sep 08". */
export function dayLabel(day: Date, now: Date = new Date()): string {
  const today = startOfDay(now)
  if (sameDay(day, today)) return 'Today'
  if (sameDay(day, addDays(today, 1))) return 'Tomorrow'
  return shortDay(day)
}

/** When the event next starts, or null when the service gave no date. */
export function eventStart(event: EventData): Date | null {
  const at = event.next_start_at != null ? Date.parse(event.next_start_at) : NaN
  return Number.isNaN(at) ? null : new Date(at)
}

/** A live event's "Started 3 hour ago"; else its start, as "4:00PM" or "TUE, SEP 08 @ 4:00PM". */
export function eventTimeText(event: EventData, onlyHours: boolean, now: Date = new Date()): string {
  const start = eventStart(event)
  if (start == null) return ''
  if (event.live) {
    const elapsed = now.getTime() - start.getTime()
    if (elapsed >= DAY) return `Started ${Math.trunc(elapsed / DAY)} day ago`
    if (elapsed >= HOUR) return `Started ${Math.trunc(elapsed / HOUR)} hour ago`
    return `Started ${Math.trunc(elapsed / MINUTE)} min ago`
  }
  return (onlyHours ? clock(start) : `${shortDay(start)} @ ${clock(start)}`).toUpperCase()
}

/** "TUE, SEP 08". */
export function eventDayText(event: EventData): string {
  const start = eventStart(event)
  return start == null ? '' : shortDay(start).toUpperCase()
}

/** One occurrence of a recurrent event: "Wednesday, Aug 30 from 04:00pm to 05:00pm (UTC-3)". */
export function scheduleText(start: Date, durationMs: number): string {
  const end = new Date(start.getTime() + durationMs)
  const offsetHours = Math.trunc(-start.getTimezoneOffset() / 60)
  return `${WEEKDAYS[start.getDay()]}, ${MONTHS[start.getMonth()]} ${two(start.getDate())} from ${paddedClock(start)} to ${paddedClock(end)} (UTC${offsetHours >= 0 ? '+' : ''}${offsetHours})`
}

/** The occurrences the detail lists: those from the next start on. */
export function scheduleDates(event: EventData): Date[] {
  const next = eventStart(event)
  if (next == null) return []
  return event.recurrent_dates
    .map((d) => new Date(d))
    .filter((d) => !Number.isNaN(d.getTime()) && d.getTime() >= next.getTime())
}

/** A date as the events service takes it in `from` and `to`: UTC, to the second. */
export function rangeParam(date: Date): string {
  return `${date.toISOString().slice(0, 19)}Z`
}

/** The range a calendar of `days` days from `startDay` reads. It starts a day early; the page
 *  drops what falls before `startDay`. */
export function calendarRange(startDay: Date, days: number): { from: string; to: string } {
  return { from: rangeParam(addDays(startDay, -1)), to: rangeParam(new Date(addDays(startDay, days).getTime() - 1000)) }
}

/** The range one day reads. */
export function dayRange(day: Date): { from: string; to: string } {
  return { from: rangeParam(day), to: rangeParam(new Date(addDays(day, 1).getTime() - 1000)) }
}

export type EventCardSize = 'big' | 'small'

/** An event gets the big card when it is highlighted, live, reminded, or held by a community the
 *  user is a member of. `reminded` is the reminder as it was when the list was read. */
export function eventCardSize(event: EventData, reminded: boolean, inMemberCommunity: boolean): EventCardSize {
  return event.highlighted || event.live || reminded || inMemberCommunity ? 'big' : 'small'
}

/** How many empty slots follow a column's cards: a big card fills one slot, a small one half. */
export function columnOccupancy(sizes: EventCardSize[]): number {
  return sizes.reduce((sum, size) => sum + (size === 'big' ? 1 : 0.5), 0)
}
export function emptySlotCount(occupancy: number): number {
  if (occupancy <= 0.5) return 3
  if (occupancy <= 1.5) return 2
  return 1
}
/** A column offers its scroll arrows from this occupancy up. */
export const SCROLL_ARROWS_OCCUPANCY = 1.5

// ---- links -----------------------------------------------------------------

const CAMPAIGN = 'utm_campaign=discover'
const out = (url: string): string => `${withUtm(url)}&${CAMPAIGN}`

export function createEventUrl(): string {
  return out(`https://${BASE_DOMAIN}/whats-on/new-event`)
}

/** The link that drops a visitor at the event's place, crediting `referrer` (the sharer's address). */
export function eventJumpLink(event: EventData, referrer: string | null): string {
  const link = event.world ? `https://${BASE_DOMAIN}/jump/?realm=${event.server ?? ''}` : `https://${BASE_DOMAIN}/jump/?position=${event.x},${event.y}`
  return referrer != null && referrer !== '' ? `${link}&referrer=${referrer}` : link
}

/** What "Copy link" copies: the jump link while the event is live, else its page. */
export function eventCopyLink(event: EventData, referrer: string | null): string {
  return event.live ? eventJumpLink(event, referrer) : `https://${BASE_DOMAIN}/whats-on/?id=${event.id}`
}

export function eventShareUrl(event: EventData, referrer: string | null): string {
  return out(`https://twitter.com/intent/tweet?text=${encodeURIComponent(event.name)}&hashtags=DCLPlace&url=${encodeURIComponent(eventCopyLink(event, referrer))}`)
}

const calendarStamp = (date: Date): string => `${date.toISOString().slice(0, 19).replace(/[-:]/g, '')}Z`

/** Google Calendar's new-event page for the event's next occurrence, or for the one at `start`. */
export function eventCalendarUrl(event: EventData, referrer: string | null, start?: Date): string | null {
  const from = start ?? eventStart(event)
  if (from == null) return null
  const finish = event.next_finish_at != null ? Date.parse(event.next_finish_at) : NaN
  const to = start != null || Number.isNaN(finish) ? new Date(from.getTime() + (event.duration ?? 0)) : new Date(finish)
  const details = `${event.description ?? ''}\n\njump in: ${eventJumpLink(event, referrer)}`
  return out(
    `https://calendar.google.com/calendar/r/eventedit?text=${encodeURIComponent(event.name)}&details=${encodeURIComponent(details)}&dates=${calendarStamp(from)}/${calendarStamp(to)}`
  )
}

/** Where the detail says the event is held. */
export function eventPlaceText(event: EventData, place: EventPlace | undefined): string {
  if (place == null) return `${event.scene_name ?? ''} (${event.x},${event.y})`
  if (event.world) return `${place.title} (${place.world_name ?? ''})`
  return `${place.title} (${event.x},${event.y})`
}

// ---- analytics -------------------------------------------------------------

export function eventAnalytics(event: EventData): Record<string, unknown> {
  return { event_id: event.id, event_name: event.name, event_coords: `(${event.x}, ${event.y})`, highlighted: event.highlighted }
}
