import type { EventData } from '../../engine/protocol'

export const UPCOMING_MAX = 10

const startOf = (e: EventData): number => Date.parse(e.next_start_at ?? e.start_at ?? '')

/** Live events keep the list's order; upcoming ones are sorted soonest first. */
export function splitEvents(events: EventData[]): { live: EventData[]; upcoming: EventData[] } {
  const live = events.filter((e) => e.live)
  const upcoming = events
    .filter((e) => !e.live)
    .sort((a, b) => startOf(a) - startOf(b))
    .slice(0, UPCOMING_MAX)
  return { live, upcoming }
}

/** How long until it starts in the coarsest unit that fits: "In 20 min", "In 2 hours", "In 3 days". */
export function startsInText(e: EventData, now: number = Date.now()): string {
  const remaining = startOf(e) - now
  if (!(remaining > 0)) return 'Starting now'
  const minutes = Math.max(1, Math.round(remaining / 60_000))
  if (minutes < 60) return `In ${minutes} min`
  const hours = Math.round(remaining / 3_600_000)
  if (hours < 24) return units(hours, 'hour')
  return units(Math.round(remaining / 86_400_000), 'day')
}

const units = (n: number, unit: string): string => `In ${n} ${n === 1 ? unit : unit + 's'}`

export const eventAttendees = (e: EventData): number => e.connected_addresses.length
