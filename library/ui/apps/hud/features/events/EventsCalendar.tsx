// The calendar: a bar of day names over one column of event cards a day, with the highlighted
// events' carousel in place of the last day when there are any.

import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { TintButton, EmptyState, MaskIcon, ScrollThumb } from '../../design'
import type { EventData, Friend } from '@explorer-core/engine/protocol'
import type { EventCommunity } from '../session/useEngineSession'
import arrowDownIcon from '../../assets/events/arrow-down.png'
import arrowLeftIcon from '../../assets/events/arrow-left.png'
import arrowRightIcon from '../../assets/events/arrow-right.png'
import arrowUpIcon from '../../assets/events/arrow-up.png'
import { EventCardBanner, EventCardBig, EventCardEmptySlot, EventCardSkeleton, EventCardSmall, friendsAtEvent } from './EventCard'
import { SCROLL_ARROWS_OCCUPANCY, addDays, columnOccupancy, dayLabel, emptySlotCount, eventCardSize, sameDay, startOfDay } from './eventFormat'
import type { EventActions } from './useEventActions'
import styles from './EventsCalendar.module.css'

const SCROLL_STEP_PX = 300
export const CAROUSEL_DELAY_MS = 10_000
const SKELETON_CARDS = 3

export interface EventCardContext {
  friends: Friend[]
  memberOf: EventCommunity[]
  remindedWhenRead: string[]
  actions: EventActions
  onOpen: (event: EventData) => void
}

function communityOf(event: EventData, memberOf: EventCommunity[]): EventCommunity | undefined {
  return event.community_id != null ? memberOf.find((c) => c.id === event.community_id) : undefined
}

function DayPill({ day, onClick }: { day: Date; onClick: () => void }): React.JSX.Element {
  const today = sameDay(day, startOfDay(new Date()))
  return (
    <span className={styles.dayCell}>
      <button type="button" className={`${styles.dayPill} ${today ? styles.dayToday : ''}`.trim()} onClick={onClick}>
        {dayLabel(day)}
      </button>
    </span>
  )
}

function Column({ day, events, loading, context }: { day: Date; events: EventData[]; loading: boolean; context: EventCardContext }): React.JSX.Element {
  const scroller = useRef<HTMLDivElement>(null)
  const [edge, setEdge] = useState({ top: true, bottom: true })
  const sizes = events.map((e) => eventCardSize(e, context.remindedWhenRead.includes(e.id), communityOf(e, context.memberOf) != null))
  const occupancy = columnOccupancy(sizes)
  const ids = events.map((e) => e.id).join(',')

  const measure = (): void => {
    const el = scroller.current
    if (el == null) return
    const range = el.scrollHeight - el.clientHeight
    const next = { top: range <= 0 || el.scrollTop <= range * 0.01, bottom: range <= 0 || el.scrollTop >= range * 0.99 }
    setEdge((prev) => (prev.top === next.top && prev.bottom === next.bottom ? prev : next))
  }
  useEffect(() => {
    if (scroller.current != null) scroller.current.scrollTop = 0
    measure()
  }, [ids, loading])

  const scroll = (by: number): void => scroller.current?.scrollBy({ top: by, behavior: 'smooth' })
  const arrows = !loading && occupancy > SCROLL_ARROWS_OCCUPANCY

  return (
    <section className={styles.column} aria-label={`${dayLabel(day)} events`} aria-busy={loading}>
      <span className={styles.columnOutline} />
      {loading ? (
        <div className={styles.columnLoading}>
          {Array.from({ length: SKELETON_CARDS }, (_, i) => (
            <EventCardSkeleton key={i} />
          ))}
        </div>
      ) : (
        <>
          <div ref={scroller} className={styles.columnScroll} onScroll={measure}>
            {events.map((event, i) => {
              const community = communityOf(event, context.memberOf)
              const Card = sizes[i] === 'big' ? EventCardBig : EventCardSmall
              return (
                <Card
                  key={event.id}
                  event={event}
                  friends={friendsAtEvent(event, context.friends)}
                  community={community}
                  actions={context.actions}
                  onOpen={() => context.onOpen(event)}
                />
              )
            })}
            {Array.from({ length: emptySlotCount(occupancy) }, (_, i) => (
              <EventCardEmptySlot key={`empty-${i}`} onClick={() => context.actions.createEvent(false)} />
            ))}
          </div>
          <ScrollThumb target={scroller} tone="dark" className={styles.columnThumb} />
          {arrows && !edge.top && (
            <button type="button" className={`${styles.more} ${styles.moreUp}`} aria-label="Earlier events" onClick={() => scroll(-SCROLL_STEP_PX)}>
              <MaskIcon src={arrowUpIcon} size={20} />
            </button>
          )}
          {arrows && !edge.bottom && (
            <button type="button" className={`${styles.more} ${styles.moreDown}`} aria-label="More events" onClick={() => scroll(SCROLL_STEP_PX)}>
              <MaskIcon src={arrowDownIcon} size={20} />
            </button>
          )}
        </>
      )}
    </section>
  )
}

function Carousel({ events, context }: { events: EventData[]; context: EventCardContext }): React.JSX.Element {
  const [index, setIndex] = useState(0)
  const current = Math.min(index, events.length - 1)
  const count = events.length
  // each event stays its time, counted again from a pick
  useEffect(() => {
    if (count < 2) return
    const timer = setTimeout(() => setIndex((current + 1) % count), CAROUSEL_DELAY_MS)
    return () => clearTimeout(timer)
  }, [current, count])
  const event = events[current]
  return (
    <section className={styles.carousel} aria-label="Highlighted events">
      <EventCardBanner event={event} friends={friendsAtEvent(event, context.friends)} actions={context.actions} onOpen={() => context.onOpen(event)} withDots={count > 1} />
      {count > 1 && (
        <div className={styles.dots}>
          {events.map((e, i) => (
            <button
              key={e.id}
              type="button"
              className={`${styles.dot} ${i === current ? styles.dotSelected : ''}`.trim()}
              aria-label={`Highlighted event ${i + 1} of ${count}`}
              aria-current={i === current}
              onClick={() => setIndex(i)}
            />
          ))}
        </div>
      )}
    </section>
  )
}

export function EventsCalendar({
  startDay,
  columns,
  highlighted,
  loading,
  failed,
  arrowsLocked,
  context,
  onStartDay,
  onDay,
  onRetry
}: {
  startDay: Date
  columns: EventData[][]
  highlighted: EventData[]
  loading: boolean
  failed: boolean
  arrowsLocked: boolean
  context: EventCardContext
  onStartDay: (day: Date) => void
  onDay: (day: Date) => void
  onRetry: () => void
}): React.JSX.Element {
  const days = columns.length
  const atToday = sameDay(startDay, startOfDay(new Date()))
  return (
    <div className={styles.calendar}>
      <div className={styles.dayBar}>
        {columns.map((_, i) => {
          const day = addDays(startDay, i)
          return <DayPill key={day.getTime()} day={day} onClick={() => onDay(day)} />
        })}
        <TintButton
          radius="pill"
          tone="clear"
          className={`${styles.rangeArrow} ${styles.rangePrev}`}
          aria-label="Previous days"
          disabled={arrowsLocked || atToday}
          onClick={() => onStartDay(addDays(startDay, -days))}
        >
          <MaskIcon src={arrowLeftIcon} size={20} />
        </TintButton>
        <TintButton
          radius="pill"
          tone="clear"
          className={`${styles.rangeArrow} ${styles.rangeNext}`}
          aria-label="Next days"
          disabled={arrowsLocked}
          onClick={() => onStartDay(addDays(startDay, days))}
        >
          <MaskIcon src={arrowRightIcon} size={20} />
        </TintButton>
      </div>
      <div className={styles.columns}>
        {failed ? (
          <div className={styles.failed} style={{ '--days': days } as CSSProperties}>
            <EmptyState variant="inline" tone="error" title="Couldn't load events" subtitle="There was an error loading events. Please try again." actions={[{ label: 'Retry', onClick: onRetry }]} />
          </div>
        ) : (
          columns.map((events, i) => <Column key={addDays(startDay, i).getTime()} day={addDays(startDay, i)} events={events} loading={loading} context={context} />)
        )}
      </div>
      {highlighted.length > 0 && <Carousel events={highlighted} context={context} />}
    </div>
  )
}
