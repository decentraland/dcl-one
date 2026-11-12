import { describe, expect, it } from 'vitest'
import { addDays, startOfDay } from '../features/events/eventFormat'
import { calendarColumns, emptyEventsStore, eventsReducer, placeIds, type EventsStore } from '../features/events/eventsStore'
import type { EventData } from '../engine/protocol'
import { eventAt } from './eventFixtures'

const now = new Date(2026, 8, 8, 12, 0, 0)
const today = startOfDay(now)
const at = (dayOffset: number, hour: number): Date => new Date(today.getFullYear(), today.getMonth(), today.getDate() + dayOffset, hour)

const loaded = (store: EventsStore, query: 'range' | 'live' | 'day' | 'highlighted', events: EventData[]): EventsStore => eventsReducer(store, { type: 'loaded', query, events })
const ids = (columns: EventData[][]): string[][] => columns.map((c) => c.map((e) => e.id))

describe('events store', () => {
  it('puts each event under the local day it next starts on, and drops those before the first day', () => {
    const store = loaded(emptyEventsStore(), 'range', [eventAt('yesterday', at(-1, 23)), eventAt('a', at(0, 0)), eventAt('b', at(0, 23)), eventAt('c', at(2, 9)), eventAt('late', at(5, 1))])
    expect(ids(calendarColumns(store, today, 5, now))).toEqual([['a', 'b'], [], ['c'], [], []])
  })

  it('adds events live since an earlier day to today, after the live events that started today', () => {
    let store = loaded(emptyEventsStore(), 'range', [eventAt('liveToday', at(0, 11), { live: true }), eventAt('later', at(0, 20))])
    store = loaded(store, 'live', [eventAt('liveToday', at(0, 11), { live: true }), eventAt('liveSinceYesterday', at(-1, 9), { live: true })])
    expect(ids(calendarColumns(store, today, 2, now))).toEqual([['liveToday', 'liveSinceYesterday', 'later'], []])
    // a calendar that starts on another day leaves them out
    expect(ids(calendarColumns(store, addDays(today, 1), 2, now))).toEqual([[], []])
  })

  it('keeps one copy of an event for every list, so a reminder shows in each', () => {
    const event = eventAt('a', at(1, 10), { total_attendees: 2 })
    let store = loaded(loaded(emptyEventsStore(), 'range', [event]), 'day', [event])
    store = eventsReducer(store, { type: 'attendingSent', eventId: 'a' })
    expect(store.pending).toEqual(['a'])
    store = eventsReducer(store, { type: 'attendingDone', eventId: 'a', attending: true, failed: false })
    expect(store.pending).toEqual([])
    expect(store.byId.a).toMatchObject({ attending: true, total_attendees: 3 })
    expect(calendarColumns(store, today, 2, now)[1][0].attending).toBe(true)
  })

  it('leaves the reminder as it was when the service refuses', () => {
    let store = loaded(emptyEventsStore(), 'range', [eventAt('a', at(1, 10))])
    store = eventsReducer(eventsReducer(store, { type: 'attendingSent', eventId: 'a' }), { type: 'attendingDone', eventId: 'a', attending: true, failed: true })
    expect(store.pending).toEqual([])
    expect(store.byId.a.attending).toBe(false)
  })

  it('lists each place once', () => {
    expect(placeIds([eventAt('a', now, { place_id: 'p1' }), eventAt('b', now, { place_id: 'p1' }), eventAt('c', now), eventAt('d', now, { place_id: 'p2' })])).toEqual(['p1', 'p2'])
  })
})
