// The lobby's analytics: a visit (opened, closed with its length), the place and event cards opened or
// jumped into from each section, a jump to a friend, and the Places or Events page opened from it.
import { track } from '../../lib/analytics'

const SOURCE = 'lobby'

/** The section a card was in. */
export type CardOrigin = 'landing' | 'recent' | 'recommended' | 'live_events' | 'upcoming_events'

let startup = false
let openedAt = 0

export function lobbyOpened(isStartup: boolean, now: number = performance.now()): void {
  startup = isStartup
  openedAt = now
  track('lobby_opened', { source: SOURCE, is_startup: startup })
}

export function lobbyClosed(now: number = performance.now()): void {
  track('lobby_closed', { source: SOURCE, is_startup: startup, duration_sec: (now - openedAt) / 1000 })
}

export interface TrackedPlace {
  id: string
  title: string
  base_position?: string
  world_name?: string
  highlighted?: boolean
}

const placePayload = (p: TrackedPlace, origin: CardOrigin): Record<string, unknown> => ({
  place_id: p.id,
  place_name: p.title,
  place_coords: p.world_name && p.world_name.trim() !== '' ? p.world_name : (p.base_position ?? ''),
  highlighted: p.highlighted === true,
  from_section: origin,
  source: SOURCE,
  is_startup: startup
})

export const placeCardClicked = (p: TrackedPlace, origin: CardOrigin): void => track('place_card_clicked', placePayload(p, origin))
export const placeJumpedIn = (p: TrackedPlace, origin: CardOrigin): void => track('place_jumped_in', placePayload(p, origin))

export interface TrackedEvent {
  id: string
  name: string
  x: number
  y: number
  highlighted?: boolean
}

const eventPayload = (e: TrackedEvent, origin: CardOrigin): Record<string, unknown> => ({
  event_id: e.id,
  event_name: e.name,
  event_coords: `(${e.x}, ${e.y})`,
  highlighted: e.highlighted === true,
  from_section: origin,
  source: SOURCE,
  is_startup: startup
})

export const eventCardClicked = (e: TrackedEvent, origin: CardOrigin): void => track('event_card_clicked', eventPayload(e, origin))
export const eventJumpedIn = (e: TrackedEvent, origin: CardOrigin): void => track('event_jumped_in', eventPayload(e, origin))

export function friendJoined(address: string, parcel: { x: number; y: number }): void {
  track('jump_to_friend_clicked', { receiver_id: address, friend_position: `(${parcel.x}, ${parcel.y})`, source: SOURCE, is_startup: startup })
}

export function sectionOpened(section: 'places' | 'events'): void {
  track(section === 'places' ? 'places_section_opened' : 'events_section_opened', { source: SOURCE, is_startup: startup })
}
