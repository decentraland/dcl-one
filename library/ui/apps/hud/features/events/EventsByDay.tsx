// One day's events, in a grid of big cards.

import { TintButton, EmptyState, MaskIcon } from '../../design'
import type { EventData } from '@explorer-core/engine/protocol'
import arrowLeftIcon from '../../assets/events/arrow-left.png'
import emptyArt from '../../assets/events/empty-events-img.png'
import { EventCardBig, EventCardSkeleton, friendsAtEvent } from './EventCard'
import { dayLabel } from './eventFormat'
import type { EventCardContext } from './EventsCalendar'
import type { QueryStatus } from './eventsStore'
import styles from './EventsByDay.module.css'

const SKELETON_CARDS = 10

export function EventsByDay({
  day,
  events,
  status,
  context,
  onBack,
  onNextDay,
  onRetry
}: {
  day: Date
  events: EventData[]
  status: QueryStatus
  context: EventCardContext
  onBack: () => void
  onNextDay: () => void
  onRetry: () => void
}): React.JSX.Element {
  const loaded = status === 'loaded'
  const label = dayLabel(day)
  return (
    <div className={styles.byDay}>
      <div className={styles.top}>
        <TintButton radius={12} tone="dark" className={styles.back} aria-label="Back to the calendar" onClick={onBack}>
          <MaskIcon src={arrowLeftIcon} size={20} />
        </TintButton>
        <h2 className={styles.counter}>{loaded ? `${label} (${events.length})` : label}</h2>
      </div>
      <div className={styles.results} aria-busy={status === 'loading'}>
        {status === 'failed' ? (
          <div className={styles.center}>
            <EmptyState variant="inline" tone="error" title="Couldn't load events" subtitle="There was an error loading events. Please try again." actions={[{ label: 'Retry', onClick: onRetry }]} />
          </div>
        ) : !loaded ? (
          <div className={styles.grid}>
            {Array.from({ length: SKELETON_CARDS }, (_, i) => (
              <EventCardSkeleton key={i} wide />
            ))}
          </div>
        ) : events.length === 0 ? (
          <div className={styles.empty}>
            <img src={emptyArt} alt="" width={100} height={100} draggable={false} />
            <p className={styles.emptyTitle}>No plans here yet</p>
            <p className={styles.emptyText}>Skip ahead and see what’s coming up next.</p>
            <TintButton radius={12} tone="brand" className={styles.nextDay} onClick={onNextDay}>
              GO TO NEXT DAY
            </TintButton>
          </div>
        ) : (
          <div className={styles.grid}>
            {events.map((event) => (
              <EventCardBig
                key={event.id}
                wide
                event={event}
                friends={friendsAtEvent(event, context.friends)}
                community={event.community_id != null ? context.memberOf.find((c) => c.id === event.community_id) : undefined}
                actions={context.actions}
                onOpen={() => context.onOpen(event)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
