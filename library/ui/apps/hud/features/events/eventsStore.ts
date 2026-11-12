// The events the Events page holds, by id, and which of them each of its lists shows. One copy of
// an event serves the calendar, the day view, the banner and the detail, so a reminder set in one
// shows in all.

import type { EventData, EventPlace } from '@explorer-core/engine/protocol'
import { addDays, eventStart, sameDay, startOfDay } from './eventFormat'

/** The lists the page reads: a calendar's range, the events live now, the highlighted ones, and
 *  one day's. */
export type EventsQuery = 'range' | 'live' | 'highlighted' | 'day'
export type QueryStatus = 'idle' | 'loading' | 'loaded' | 'failed'

export interface EventsQueryState {
  status: QueryStatus
  ids: string[]
}

export interface EventsStore {
  byId: Record<string, EventData>
  queries: Record<EventsQuery, EventsQueryState>
  places: Record<string, EventPlace>
  /** The events that were reminded when their list was read. A card keeps the size it was given
   *  then: a reminder set afterwards changes its background, not its size. */
  remindedWhenRead: string[]
  /** Events whose reminder change is on its way to the service. */
  pending: string[]
}

const idle = (): EventsQueryState => ({ status: 'idle', ids: [] })

export const emptyEventsStore = (): EventsStore => ({
  byId: {},
  queries: { range: idle(), live: idle(), highlighted: idle(), day: idle() },
  places: {},
  remindedWhenRead: [],
  pending: []
})

export type EventsAction =
  | { type: 'loading'; query: EventsQuery }
  | { type: 'loaded'; query: EventsQuery; events: EventData[] }
  | { type: 'failed'; query: EventsQuery }
  | { type: 'idle'; query: EventsQuery }
  | { type: 'attendingSent'; eventId: string }
  | { type: 'attendingDone'; eventId: string; attending: boolean; failed: boolean }
  | { type: 'places'; places: EventPlace[] }
  | { type: 'reset' }

export function eventsReducer(store: EventsStore, action: EventsAction): EventsStore {
  switch (action.type) {
    case 'loading':
      return { ...store, queries: { ...store.queries, [action.query]: { status: 'loading', ids: [] } } }
    case 'loaded': {
      const byId = { ...store.byId }
      for (const event of action.events) byId[event.id] = event
      const read = new Set(action.events.map((e) => e.id))
      const remindedWhenRead = [...store.remindedWhenRead.filter((id) => !read.has(id)), ...action.events.filter((e) => e.attending).map((e) => e.id)]
      return { ...store, byId, remindedWhenRead, queries: { ...store.queries, [action.query]: { status: 'loaded', ids: action.events.map((e) => e.id) } } }
    }
    case 'failed':
      return { ...store, queries: { ...store.queries, [action.query]: { status: 'failed', ids: [] } } }
    case 'idle':
      return { ...store, queries: { ...store.queries, [action.query]: idle() } }
    case 'attendingSent':
      return store.pending.includes(action.eventId) ? store : { ...store, pending: [...store.pending, action.eventId] }
    case 'attendingDone': {
      const pending = store.pending.filter((id) => id !== action.eventId)
      const event = store.byId[action.eventId]
      if (action.failed || event == null || event.attending === action.attending) return { ...store, pending }
      const total_attendees = Math.max(0, event.total_attendees + (action.attending ? 1 : -1))
      return { ...store, pending, byId: { ...store.byId, [event.id]: { ...event, attending: action.attending, total_attendees } } }
    }
    case 'places': {
      const places = { ...store.places }
      for (const place of action.places) places[place.id] = place
      return { ...store, places }
    }
    case 'reset':
      return emptyEventsStore()
  }
}

const events = (store: EventsStore, query: EventsQuery): EventData[] => store.queries[query].ids.flatMap((id) => (store.byId[id] != null ? [store.byId[id]] : []))

export const highlightedEvents = (store: EventsStore): EventData[] => events(store, 'highlighted')
export const dayEvents = (store: EventsStore): EventData[] => events(store, 'day')

/** The calendar's columns: the range's events under the local day each next starts on, from
 *  `startDay` for `days` days. When the calendar starts today, events live since an earlier day
 *  join today's column, after the live events that started today. */
export function calendarColumns(store: EventsStore, startDay: Date, days: number, now: Date = new Date()): EventData[][] {
  const columns: EventData[][] = Array.from({ length: days }, () => [])
  for (const event of events(store, 'range')) {
    const start = eventStart(event)
    if (start == null) continue
    const index = columns.findIndex((_, i) => sameDay(start, addDays(startDay, i)))
    if (index >= 0) columns[index].push(event)
  }
  if (sameDay(startDay, startOfDay(now)) && days > 0) {
    let at = columns[0].filter((e) => e.live).length
    for (const event of events(store, 'live')) {
      const start = eventStart(event)
      if (start != null && start.getTime() < startDay.getTime()) columns[0].splice(at++, 0, event)
    }
  }
  return columns
}

/** The ids of the places `list` is held at. */
export function placeIds(list: EventData[]): string[] {
  return [...new Set(list.flatMap((e) => (e.place_id != null && e.place_id !== '' ? [e.place_id] : [])))]
}
