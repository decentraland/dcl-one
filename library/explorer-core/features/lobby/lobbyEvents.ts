// The lobby's events: one list from the events service, split into the live ones and the next upcoming
// ones, with the texts and links their cards show.
import type { LobbyEvent } from '../../engine/protocol'

export type { LobbyEvent }

/** Upcoming events the lobby lists at most. */
export const UPCOMING_MAX = 10

const startOf = (e: LobbyEvent): number => Date.parse(e.next_start_at ?? e.start_at)

/** The live events as listed, and the next upcoming ones, soonest first. */
export function splitEvents(events: LobbyEvent[]): { live: LobbyEvent[]; upcoming: LobbyEvent[] } {
  const live = events.filter((e) => e.live)
  const upcoming = events
    .filter((e) => !e.live)
    .sort((a, b) => startOf(a) - startOf(b))
    .slice(0, UPCOMING_MAX)
  return { live, upcoming }
}

/** How long until it starts in the coarsest unit that fits: "In 20 min", "In 2 hours", "In 3 days". */
export function startsInText(e: LobbyEvent, now: number = Date.now()): string {
  const remaining = startOf(e) - now
  if (!(remaining > 0)) return 'Starting now'
  const minutes = Math.max(1, Math.round(remaining / 60_000))
  if (minutes < 60) return `In ${minutes} min`
  const hours = Math.round(remaining / 3_600_000)
  if (hours < 24) return units(hours, 'hour')
  return units(Math.round(remaining / 86_400_000), 'day')
}

const units = (n: number, unit: string): string => `In ${n} ${n === 1 ? unit : unit + 's'}`

/** The people in the event's scene now. */
export const eventAttendees = (e: LobbyEvent): number => e.connected_addresses?.length ?? 0

/** Where Jump In lands for the event, as a link the site opens: its world or its parcel. */
export function eventJumpInLink(e: LobbyEvent, site: (path: string) => string, referrer?: string | null): string {
  const base = e.world && e.server ? site(`/jump/?realm=${e.server}`) : site(`/jump/?position=${e.x},${e.y}`)
  return referrer ? `${base}&referrer=${referrer}` : base
}

/** The link Copy and Share hand out: the jump-in link while live, else the event's page. */
export function eventCopyLink(e: LobbyEvent, site: (path: string) => string, referrer?: string | null): string {
  return e.live ? eventJumpInLink(e, site, referrer) : site(`/whats-on/?id=${e.id}`)
}

export function eventShareLink(e: LobbyEvent, site: (path: string) => string, referrer?: string | null): string {
  return `https://twitter.com/intent/tweet?text=${e.name}&hashtags=DCLPlace&url=${encodeURIComponent(eventCopyLink(e, site, referrer))}`
}

const calendarStamp = (iso: string): string => new Date(iso).toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')

/** A Google Calendar draft for the event, with the jump-in link in its details. */
export function eventCalendarLink(e: LobbyEvent, site: (path: string) => string, referrer?: string | null): string {
  const start = e.next_start_at ?? e.start_at
  const finish = e.next_finish_at ?? e.finish_at ?? start
  const details = `${e.description ?? ''}\n\njump in: ${encodeURIComponent(eventJumpInLink(e, site, referrer))}`
  return `https://calendar.google.com/calendar/r/eventedit?text=${e.name}&details=${details}&dates=${calendarStamp(start)}/${calendarStamp(finish)}`
}
