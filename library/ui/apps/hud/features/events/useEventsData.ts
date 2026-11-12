// The Events page's data: its requests to the bridge (signed reads of the events service, the
// reminder on an event, the places events are held at) and their answers, kept in one store.

import { useCallback, useEffect, useMemo, useReducer, useRef } from 'react'
import { showToast } from '../../design/Toast'
import type { LoginDriver } from '@explorer-core/engine/driver'
import type { EventData, GetEventsRequest, SceneToPage } from '@explorer-core/engine/protocol'
import { calendarRange, dayRange, startOfDay, sameDay } from './eventFormat'
import { emptyEventsStore, eventsReducer, placeIds, type EventsQuery, type EventsStore } from './eventsStore'

/** How long a request waits for the bridge's answer before it counts as failed. An answer that
 *  arrives later still applies, unless a newer request took its place. */
export const EVENTS_REPLY_TIMEOUT_MS = 10_000
export const HIGHLIGHTED_EVENTS_LIMIT = 10

const LOAD_ERROR = 'There was an error loading events. Please try again.'
const LIVE_LOAD_ERROR = 'There was an error loading live events. Please try again.'
const REMINDER_ERROR = 'There was an error changing your interest on the event. Please try again.'

export interface EventsData {
  store: EventsStore
  loadCalendar: (startDay: Date, days: number, withHighlighted: boolean) => void
  loadDay: (day: Date) => void
  /** Set or clear the user's reminder; the store changes when the service confirms. */
  setAttending: (event: EventData, attending: boolean) => void
  clear: () => void
}

type Pending = { request: number; timer: ReturnType<typeof setTimeout> | null }

export function useEventsData(driverRef: React.RefObject<LoginDriver | null>, onReminderSet?: (event: EventData) => void): EventsData & { onMessage: (msg: SceneToPage) => void } {
  const [store, dispatch] = useReducer(eventsReducer, undefined, emptyEventsStore)
  const storeRef = useRef(store)
  storeRef.current = store
  const seq = useRef(0)
  const waiting = useRef<Partial<Record<EventsQuery, Pending>>>({})
  const reminderTimers = useRef(new Map<string, ReturnType<typeof setTimeout>>())
  const onReminderSetRef = useRef(onReminderSet)
  onReminderSetRef.current = onReminderSet

  const fail = useCallback((query: EventsQuery): void => {
    dispatch({ type: 'failed', query })
    // the banner is optional: without it the calendar shows one more day
    if (query === 'range' || query === 'day') showToast(LOAD_ERROR, { tone: 'error' })
    else if (query === 'live') showToast(LIVE_LOAD_ERROR, { tone: 'error' })
  }, [])

  const stop = useCallback((query: EventsQuery): void => {
    const pending = waiting.current[query]
    if (pending?.timer != null) clearTimeout(pending.timer)
    delete waiting.current[query]
  }, [])

  const read = useCallback(
    (query: EventsQuery, params: Omit<GetEventsRequest, 'kind' | 'request'>): void => {
      stop(query)
      const request = ++seq.current
      const pending: Pending = { request, timer: null }
      pending.timer = setTimeout(() => {
        pending.timer = null
        fail(query)
      }, EVENTS_REPLY_TIMEOUT_MS)
      waiting.current[query] = pending
      dispatch({ type: 'loading', query })
      driverRef.current?.send({ kind: 'getEvents', request, ...params })
    },
    [driverRef, fail, stop]
  )

  const loadCalendar = useCallback(
    (startDay: Date, days: number, withHighlighted: boolean): void => {
      read('range', calendarRange(startDay, days))
      if (sameDay(startDay, startOfDay(new Date()))) read('live', { list: 'live' })
      else {
        stop('live')
        dispatch({ type: 'idle', query: 'live' })
      }
      if (withHighlighted) read('highlighted', { list: 'highlight', limit: HIGHLIGHTED_EVENTS_LIMIT, offset: 0 })
    },
    [read, stop]
  )

  const loadDay = useCallback((day: Date): void => read('day', dayRange(day)), [read])

  const settleReminder = useCallback((eventId: string, attending: boolean, failed: boolean): void => {
    const timer = reminderTimers.current.get(eventId)
    if (timer == null) return
    clearTimeout(timer)
    reminderTimers.current.delete(eventId)
    const before = storeRef.current.byId[eventId]
    dispatch({ type: 'attendingDone', eventId, attending, failed })
    if (failed) showToast(REMINDER_ERROR, { tone: 'error' })
    else if (before != null) onReminderSetRef.current?.({ ...before, attending })
  }, [])

  const setAttending = useCallback(
    (event: EventData, attending: boolean): void => {
      if (reminderTimers.current.has(event.id)) return
      reminderTimers.current.set(
        event.id,
        setTimeout(() => settleReminder(event.id, attending, true), EVENTS_REPLY_TIMEOUT_MS)
      )
      dispatch({ type: 'attendingSent', eventId: event.id })
      driverRef.current?.send({ kind: 'setEventAttending', eventId: event.id, attending })
    },
    [driverRef, settleReminder]
  )

  const clear = useCallback((): void => {
    for (const query of Object.keys(waiting.current) as EventsQuery[]) stop(query)
    reminderTimers.current.forEach((timer) => clearTimeout(timer))
    reminderTimers.current.clear()
    dispatch({ type: 'reset' })
  }, [stop])

  const onMessage = useCallback(
    (msg: SceneToPage): void => {
      if (msg.kind === 'events') {
        const query = (Object.keys(waiting.current) as EventsQuery[]).find((q) => waiting.current[q]?.request === msg.request)
        if (query == null) return
        const pending = waiting.current[query]
        const timedOut = pending?.timer == null
        if (pending?.timer != null) clearTimeout(pending.timer)
        if (pending != null) pending.timer = null
        if (msg.failed === true) {
          // a request that already ran out of time has shown its error
          if (!timedOut) fail(query)
          return
        }
        dispatch({ type: 'loaded', query, events: msg.events })
        const unknown = placeIds(msg.events).filter((id) => storeRef.current.places[id] == null)
        if (unknown.length > 0) driverRef.current?.send({ kind: 'getEventPlaces', request: ++seq.current, placeIds: unknown })
      } else if (msg.kind === 'eventAttending') {
        settleReminder(msg.eventId, msg.attending, msg.failed === true)
      } else if (msg.kind === 'eventPlaces') {
        // optional: without a place the detail names the event's scene
        if (msg.failed !== true) dispatch({ type: 'places', places: msg.places })
      }
    },
    [driverRef, fail, settleReminder]
  )

  useEffect(() => clear, [clear])

  return useMemo(() => ({ store, loadCalendar, loadDay, setAttending, clear, onMessage }), [store, loadCalendar, loadDay, setAttending, clear, onMessage])
}
