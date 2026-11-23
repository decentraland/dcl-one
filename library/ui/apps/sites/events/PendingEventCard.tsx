import type { MouseEvent } from 'react'
import { EventCard, type EventCardLinkComponent } from '../../../components/EventCard/EventCard'
import { adminCopy } from './admin-copy'
import type { EventCardTime } from './types'
import styles from './PendingEventCard.module.css'

export type PendingEventStatus = 'pending' | 'approved' | 'rejected'

export type PendingEventCardProps = {
  title: string
  image: string | null
  /** The review's address; a click on the title opens the review in place through `onOpen`. */
  href: string
  onOpen: (event: MouseEvent<HTMLAnchorElement>) => void
  status: PendingEventStatus
  /** "Today", "Tomorrow", "In 3 Days" or "NOV 19"; none for an event without a start. */
  dateLabel?: string
  creatorName?: string
  time?: EventCardTime
  linkComponent?: EventCardLinkComponent
}

/**
 * A card of the moderation queue, as decentraland.org's PendingEventCard: the events page's day card
 * with the start day and the review status over its cover. A pending event shows faded.
 */
export function PendingEventCard({
  title,
  image,
  href,
  onOpen,
  status,
  dateLabel,
  creatorName,
  time,
  linkComponent
}: PendingEventCardProps) {
  return (
    <div className={styles.root} data-status={status} data-cover={image ? undefined : 'none'}>
      <div className={styles.chips}>
        {dateLabel ? <span className={styles.chip}>{dateLabel}</span> : <span />}
        <span className={styles.chip} data-status={status}>
          {adminCopy.pending.status[status]}
        </span>
      </div>
      <EventCard
        variant="day"
        title={title}
        href={href}
        onOpen={onOpen}
        image={image}
        creator={creatorName ? { name: creatorName } : undefined}
        byLabel={adminCopy.pending.by}
        time={time}
        linkComponent={linkComponent}
      />
    </div>
  )
}
