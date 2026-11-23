import { useId, type Ref } from 'react'

import { Button, Spinner, Tag } from '../../../index'
import { GeneralCalendar, GeneralVerified } from '../../../foundations/icons'

import { UpcomingCard } from '../events/cards'
import type { UpcomingEventCardProps } from '../events/types'
import { useBrokenImage } from '../useBrokenImage'
import styles from './CommunityLists.module.css'

export type CommunityListStatus = 'pending' | 'error' | 'ready'

/** The page after the last one shown: none, more to read, on its way, or failed. */
export type CommunityListMore = {
  state: 'none' | 'idle' | 'loading' | 'error'
  /** No IntersectionObserver: a Load more button replaces the automatic read. */
  manual: boolean
  /** The element whose arrival in view reads the next page. */
  sentinelRef: Ref<HTMLDivElement>
  onLoadMore: () => void
}

function MoreFooter({ more, noun }: { more: CommunityListMore; noun: string }) {
  if (more.state === 'error') {
    return (
      <div className={styles.retry} role="alert">
        <p className={styles.empty}>We couldn&apos;t load more {noun}.</p>
        <Button variant="secondary" size="sm" onClick={more.onLoadMore}>
          Try again
        </Button>
      </div>
    )
  }
  if (more.state === 'none') return null
  return (
    <div ref={more.sentinelRef} className={styles.sentinel}>
      {more.state === 'loading' ? (
        <Spinner variant="light" label={`Loading more ${noun}`} />
      ) : more.manual ? (
        <Button variant="secondary" size="sm" onClick={more.onLoadMore}>
          Load more
        </Button>
      ) : null}
    </div>
  )
}

function FirstPageFailed({ noun, onRetry }: { noun: string; onRetry: () => void }) {
  return (
    <div className={styles.retry} role="alert">
      <p className={styles.empty}>We couldn&apos;t load the {noun} right now.</p>
      <Button variant="secondary" size="sm" onClick={onRetry}>
        Try again
      </Button>
    </div>
  )
}

export type CommunityMemberRow = {
  address: string
  name: string
  face?: string | undefined
  /** "Owner", "Moderator", "Member". */
  role: string
  claimedName: boolean
}

export type CommunityMembersListProps = {
  total: number
  status: CommunityListStatus
  members: CommunityMemberRow[]
  onRetry: () => void
  more: CommunityListMore
  /** The list's own scroller, which the next page's read watches. */
  scrollerRef?: Ref<HTMLDivElement>
}

/** The community's members, scrolling inside a fixed height as source. */
export function CommunityMembersList({
  total,
  status,
  members,
  onRetry,
  more,
  scrollerRef
}: CommunityMembersListProps) {
  const titleId = useId()
  return (
    <section
      className={styles.section}
      aria-labelledby={titleId}
      aria-busy={status === 'pending' || undefined}
    >
      <h2 id={titleId} className={styles.title}>
        Members ({total})
      </h2>
      {status === 'pending' ? (
        <div className={styles.loader}>
          <Spinner variant="light" label="Loading members" />
        </div>
      ) : status === 'error' && members.length === 0 ? (
        <FirstPageFailed noun="members" onRetry={onRetry} />
      ) : members.length === 0 ? (
        <p className={styles.empty}>No members found</p>
      ) : (
        // Focusable so a keyboard can scroll it: its rows hold no control.
        <div
          ref={scrollerRef}
          className={styles.membersScroller}
          role="group"
          aria-labelledby={titleId}
          tabIndex={0}
        >
          <ul className={styles.members}>
            {members.map((member) => (
              <MemberRow key={member.address} member={member} />
            ))}
          </ul>
          <MoreFooter more={more} noun="members" />
        </div>
      )}
    </section>
  )
}

function MemberRow({ member }: { member: CommunityMemberRow }) {
  const { broken, ref, onError } = useBrokenImage(member.face)
  return (
    <li className={styles.member}>
      {member.face && !broken ? (
        <img
          ref={ref}
          className={styles.avatar}
          src={member.face}
          alt=""
          loading="lazy"
          onError={onError}
        />
      ) : (
        <span className={styles.avatar} aria-hidden="true">
          {member.name.charAt(0).toUpperCase()}
        </span>
      )}
      <span className={styles.memberText}>
        <span className={styles.memberName}>
          <span className={styles.memberLabel}>{member.name}</span>
          {member.claimedName ? (
            <span
              className={styles.verified}
              role="img"
              aria-label="Claimed name"
              title="Claimed name"
            >
              <GeneralVerified size={16} aria-hidden="true" />
            </span>
          ) : null}
        </span>
        <Tag variant="role">{member.role}</Tag>
      </span>
    </li>
  )
}

export type CommunityEventsListProps = {
  status: CommunityListStatus
  cards: UpcomingEventCardProps[]
  /** A failed Remind me. */
  notice?: string
  onRetry: () => void
  more: CommunityListMore
}

/** The community's upcoming events, as the events page's Upcoming cards. */
export function CommunityEventsList({
  status,
  cards,
  notice,
  onRetry,
  more
}: CommunityEventsListProps) {
  const titleId = useId()
  return (
    <section
      className={styles.section}
      aria-labelledby={titleId}
      aria-busy={status === 'pending' || undefined}
    >
      <h2 id={titleId} className={styles.title}>
        Upcoming Events
      </h2>
      {notice ? (
        <p className={styles.notice} role="alert">
          {notice}
        </p>
      ) : null}
      {status === 'pending' ? (
        <div className={styles.loader}>
          <Spinner variant="light" label="Loading events" />
        </div>
      ) : status === 'error' && cards.length === 0 ? (
        <FirstPageFailed noun="events" onRetry={onRetry} />
      ) : cards.length === 0 ? (
        <div className={styles.emptyEvents}>
          <GeneralCalendar size={64} aria-hidden="true" />
          <p>No Upcoming Events</p>
        </div>
      ) : (
        <ul className={styles.events}>
          {cards.map((card) => (
            <li key={card.id}>
              <UpcomingCard {...card} />
            </li>
          ))}
        </ul>
      )}
      <MoreFooter more={more} noun="events" />
    </section>
  )
}
