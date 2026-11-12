// Events — a full-screen MainMenuShell page: the calendar of the coming days with the highlighted
// events beside it, and one day's events in a grid. Every visit opens on the calendar at today.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { TintButton, MaskIcon } from '../../design'
import type { EventData } from '@explorer-core/engine/protocol'
import { track } from '@explorer-core/lib/analytics'
import { MainMenuShell } from '../menu/MainMenuShell'
import type { EventsState, FriendsState, ProfileState } from '../session/useEngineSession'
import addIcon from '../../assets/events/add-icn.png'
import { openEventDetail } from './EventDetail'
import { addDays, dayKey, eventAnalytics, startOfDay } from './eventFormat'
import { EventsByDay } from './EventsByDay'
import { EventsCalendar, type EventCardContext } from './EventsCalendar'
import { calendarColumns, dayEvents, highlightedEvents } from './eventsStore'
import { useEventActions } from './useEventActions'
import styles from './EventsPage.module.css'

/** The days a calendar shows; the highlighted carousel takes the last one's place. */
export const CALENDAR_DAYS = 5

type View = { kind: 'calendar' } | { kind: 'day'; day: Date }

export function EventsPage({
  events,
  profile,
  friends,
  onNavigate
}: {
  events: EventsState
  profile: ProfileState
  friends: FriendsState
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  const { open, data, memberOf } = events
  const { store, loadCalendar, loadDay } = data
  const actions = useEventActions()
  const [view, setView] = useState<View>({ kind: 'calendar' })
  const [startDay, setStartDay] = useState(() => startOfDay(new Date()))

  const showCalendar = useCallback(
    (from: Date) => {
      setView({ kind: 'calendar' })
      setStartDay(from)
      loadCalendar(from, CALENDAR_DAYS, true)
    },
    [loadCalendar]
  )
  const showDay = useCallback(
    (day: Date) => {
      setView({ kind: 'day', day })
      loadDay(day)
      track('events_by_day_opened', { date: dayKey(day) })
    },
    [loadDay]
  )
  useEffect(() => {
    if (open) showCalendar(startOfDay(new Date()))
  }, [open, showCalendar])

  const highlightedStatus = store.queries.highlighted.status
  const highlighted = useMemo(() => highlightedEvents(store), [store])
  const days = highlighted.length > 0 ? CALENDAR_DAYS - 1 : CALENDAR_DAYS
  const columns = useMemo(() => calendarColumns(store, startDay, days), [store, startDay, days])
  const dayList = useMemo(() => dayEvents(store), [store])

  if (!open) return null

  const context: EventCardContext = {
    friends: friends.list,
    memberOf,
    remindedWhenRead: store.remindedWhenRead,
    actions,
    onOpen: (event: EventData) => {
      openEventDetail(event.id)
      track('event_card_clicked', { ...eventAnalytics(event), source: 'explore' })
    }
  }
  const range = store.queries.range.status
  const p = profile.data
  return (
    <MainMenuShell
      active="events"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={() => events.toggle()}
      transparentBody
    >
      <div className={styles.page}>
        <div className={styles.backdrop} />
        <header className={styles.header}>
          <div className={styles.headerFrame}>
            <h1 className={styles.title}>Events</h1>
            <TintButton radius="pill" tone="glass" className={styles.today} aria-label="Go to today" onClick={() => showCalendar(startOfDay(new Date()))}>
              Today
            </TintButton>
            <TintButton radius={12} tone="dark" className={styles.create} onClick={() => actions.createEvent(true)}>
              <MaskIcon src={addIcon} size={12} />
              CREATE EVENT
            </TintButton>
          </div>
        </header>
        <div className={styles.content}>
          <div className={styles.frame}>
            {view.kind === 'calendar' ? (
              <EventsCalendar
                startDay={startDay}
                columns={columns}
                highlighted={highlighted}
                loading={range === 'loading' || range === 'idle' || store.queries.live.status === 'loading'}
                failed={range === 'failed'}
                arrowsLocked={highlightedStatus === 'loading' || highlightedStatus === 'idle'}
                context={context}
                onStartDay={(day) => {
                  setStartDay(day)
                  loadCalendar(day, CALENDAR_DAYS, false)
                }}
                onDay={showDay}
                onRetry={() => showCalendar(startDay)}
              />
            ) : (
              <EventsByDay
                day={view.day}
                events={dayList}
                status={store.queries.day.status}
                context={context}
                onBack={() => showCalendar(startDay)}
                onNextDay={() => showDay(addDays(view.day, 1))}
                onRetry={() => showDay(view.day)}
              />
            )}
          </div>
        </div>
      </div>
    </MainMenuShell>
  )
}
