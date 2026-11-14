import { useState } from 'react'
import {
  GeneralAddToCalendar,
  GeneralBell,
  GeneralCopy,
  GeneralLive,
  GeneralPlayers,
  GeneralTime
} from '../../../../foundations/icons'
import dclLogo from '../../assets/dcl-logo.svg'
import type { EventCardCreator, EventCardTime, RemindControl } from './types'
import styles from './cards.module.css'

// TODO(shared-card): these parts back the provisional cards only; they go when the slots render
// the shared EventCard and PlaceCard.

function CreatorAvatar({ creator }: { creator: EventCardCreator }) {
  const src = creator.avatarUrl ?? (creator.foundation ? dclLogo : undefined)
  if (src) return <img className={styles.avatar} src={src} alt="" loading="lazy" />
  return (
    <span className={styles.avatar} aria-hidden="true">
      {creator.name.charAt(0).toUpperCase()}
    </span>
  )
}

/** "by <creator>"; the creator link sits above the card's covering link. */
function CreatorRow({ creator, prefix = 'by' }: { creator: EventCardCreator; prefix?: string }) {
  return (
    <div className={styles.creatorRow}>
      <CreatorAvatar creator={creator} />
      <p className={styles.by}>
        {prefix}{' '}
        {creator.href ? (
          <a className={styles.creator} href={creator.href}>
            {creator.name}
          </a>
        ) : (
          <span className={styles.creator}>{creator.name}</span>
        )}
      </p>
    </div>
  )
}

function Badges({
  live,
  users,
  usersLabel,
  inline
}: {
  live: boolean
  users?: number
  usersLabel?: string
  /** In the text flow, as in a detail hero, instead of over a cover. */
  inline?: boolean
}) {
  if (!live && users === undefined) return null
  return (
    <div className={inline ? styles.badgesInline : styles.badges}>
      {live ? (
        <span className={styles.live}>
          <GeneralLive size={16} aria-hidden="true" />
          LIVE
        </span>
      ) : null}
      {users !== undefined ? (
        <span className={styles.users}>
          <span className={styles.dot} aria-hidden="true" />
          <GeneralPlayers size={14} aria-hidden="true" />
          <span aria-hidden="true">{users}</span>
          <span className={styles.visuallyHidden}>{usersLabel ?? `${users} online`}</span>
        </span>
      ) : null}
    </div>
  )
}

function TimePill({ time }: { time: EventCardTime }) {
  return (
    <p className={styles.time}>
      <GeneralTime size={16} aria-hidden="true" />
      <time dateTime={time.dateTime}>{time.label}</time>
    </p>
  )
}

type CardActionProps = {
  href: string
  title: string
  remind?: RemindControl
  calendarHref?: string
}

/** Remind me, calendar and copy actions share the card's reserved footer area. */
function CardActions({ title, remind, calendarHref, href }: CardActionProps) {
  const [copyStatus, setCopyStatus] = useState('')
  const copy = async () => {
    try {
      if (!navigator.clipboard) throw new Error('Clipboard unavailable')
      await navigator.clipboard.writeText(new URL(href, window.location.href).href)
      setCopyStatus('Link copied.')
    } catch {
      setCopyStatus("Couldn't copy the link. Try again.")
    }
  }
  return (
    <div className={styles.actions}>
      {remind ? (
        <button
          type="button"
          className={`${styles.action} ${styles.remindAction}`}
          aria-pressed={remind.active}
          aria-label={`Remind me: ${title}`}
          disabled={remind.pending}
          onClick={(event) => {
            event.stopPropagation()
            remind.onToggle()
          }}
        >
          <GeneralBell
            size={20}
            aria-hidden="true"
            className={remind.active ? styles.reminded : undefined}
          />
          <span className={styles.remindLabel}>Remind me</span>
        </button>
      ) : null}
      {calendarHref ? (
        <a
          className={styles.action}
          href={calendarHref}
          target="_blank"
          rel="noopener noreferrer"
          aria-label={`Add to calendar: ${title}`}
          onClick={(event) => event.stopPropagation()}
        >
          <GeneralAddToCalendar size={20} aria-hidden="true" />
        </a>
      ) : null}
      <button
        type="button"
        className={styles.action}
        aria-label={`Copy link: ${title}`}
        onClick={(event) => {
          event.stopPropagation()
          void copy()
        }}
      >
        <GeneralCopy size={20} aria-hidden="true" />
      </button>
      <span role="status" className={styles.visuallyHidden}>
        {copyStatus}
      </span>
    </div>
  )
}

function CardFooter({ time, ...actions }: CardActionProps & { time: EventCardTime }) {
  return (
    <div className={styles.cardFooter}>
      <TimePill time={time} />
      <CardActions {...actions} />
    </div>
  )
}

export { Badges, CardActions, CardFooter, CreatorRow, TimePill }
