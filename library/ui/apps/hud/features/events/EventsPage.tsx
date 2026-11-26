// Events — a full-screen MainMenuShell page: the calendar of the coming days with the highlighted
// events beside it, and one day's events in a grid. Every visit opens on the calendar at today.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { EventData } from '@explorer-core/engine/protocol'
import { track } from '@explorer-core/lib/analytics'
import { MainMenuShell } from '../menu/MainMenuShell'
import { MenuBackdrop } from '../menu/MenuBackdrop'
import type { EventsState, FriendsState, ProfileState } from '../session/useEngineSession'
import addIcon from '../../assets/events/add-icn.png'
import { openEventDetail, type EventJumpIn } from './EventDetail'
import { addDays, dayKey, eventAnalytics, startOfDay } from './eventFormat'
import { EventsByDay } from './EventsByDay'
import { EventsCalendar, type EventCardContext } from './EventsCalendar'
import { calendarColumns, dayEvents, highlightedEvents } from './eventsStore'
import { useEventActions } from './useEventActions'
import styles from './EventsPage.module.css'
import { Button, MaskIcon } from '@dcl/one-ui'

/** The days a calendar shows; the highlighted carousel takes the last one's place. */
export const CALENDAR_DAYS = 5

type View = { kind: 'calendar' } | { kind: 'day'; day: Date }

export function EventsPage({
  events,
  profile,
  friends,
  compact,
  onNavigate
}: {
  events: EventsState
  profile: ProfileState
  friends: FriendsState
  /** Shown in the lobby's section modal; `onTravel` closes it once a jump goes ahead. */
  compact?: { onTravel: () => void }
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  const { open, data, memberOf } = events
  const { store, loadCalendar, loadDay, clear } = data
  const inModal = compact != null
  const shown = open || inModal
  const calendarDays = inModal ? CALENDAR_DAYS - 1 : CALENDAR_DAYS
  const eventActions = useEventActions()
  const onTravel = compact?.onTravel
  const actions = useMemo(() => {
    if (onTravel == null) return eventActions
    const jumpIn: EventJumpIn = (event) => {
      if (!eventActions.jumpIn(event)) return false
      onTravel()
      return true
    }
    return { ...eventActions, jumpIn }
  }, [eventActions, onTravel])
  // the modal leaves no events behind, unless the full page took over the store
  const pageOpen = useRef(open)
  pageOpen.current = open
  useEffect(() => {
    if (!inModal) return
    return () => {
      if (!pageOpen.current) clear()
    }
  }, [inModal, clear])
  const [view, setView] = useState<View>({ kind: 'calendar' })
  const [startDay, setStartDay] = useState(() => startOfDay(new Date()))

  const showCalendar = useCallback(
    (from: Date) => {
      setView({ kind: 'calendar' })
      setStartDay(from)
      loadCalendar(from, calendarDays, true)
    },
    [loadCalendar, calendarDays]
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
    if (shown) showCalendar(startOfDay(new Date()))
  }, [shown, showCalendar])

  const highlightedStatus = store.queries.highlighted.status
  const highlighted = useMemo(() => highlightedEvents(store), [store])
  const days = highlighted.length > 0 ? calendarDays - 1 : calendarDays
  const columns = useMemo(() => calendarColumns(store, startDay, days), [store, startDay, days])
  const dayList = useMemo(() => dayEvents(store), [store])

  if (!shown) return null

  const context: EventCardContext = {
    friends: friends.list,
    memberOf,
    remindedWhenRead: store.remindedWhenRead,
    actions,
    onOpen: (event: EventData) => {
      openEventDetail(event.id, inModal ? actions.jumpIn : undefined)
      // away from the Events page the calendar reports as the lobby's
      track('event_card_clicked', { ...eventAnalytics(event), source: inModal ? 'lobby' : 'explore' })
    }
  }
  const range = store.queries.range.status
  const p = profile.data
  const page = (
    <div className={inModal ? `${styles.page} ${styles.compact}` : styles.page}>
      <MenuBackdrop />
      <header className={styles.header}>
        <div className={styles.headerFrame}>
          <h1 className={styles.title}>Events</h1>
          <Button appearance="hud" size="bare" radius="pill" variant="glass" className={styles.today} aria-label="Go to today" onClick={() => showCalendar(startOfDay(new Date()))}>
            Today
          </Button>
          <Button appearance="hud" size="bare" radius="md" variant="scrim" className={styles.create} onClick={() => actions.createEvent(true)}>
            <MaskIcon src={addIcon} size={12} />
            CREATE EVENT
          </Button>
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
                loadCalendar(day, calendarDays, false)
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
  )
  if (inModal) return page
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
      {page}
    </MainMenuShell>
  )
}
