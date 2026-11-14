import { useEffect, useId, useState, type ReactNode } from 'react'
import {
  GeneralAddToCalendar,
  GeneralBell,
  GeneralClose,
  GeneralCopy,
  GeneralJumpIn,
  GeneralMapPin,
  GeneralPlayers,
  GeneralPublic
} from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import { Badges, CreatorRow } from './cards/parts'
import type { EventCardCreator, EventCardTime, JumpIn, RemindControl } from './cards/types'
import { Dialog } from './Dialog'
import styles from './DetailModal.module.css'

type Location = { label: string; world: boolean; jumpIn?: JumpIn }

type EventDetailModalProps = {
  title: string
  image: string | null
  live: boolean
  /** The start under the title, for an event that is not live. */
  when?: EventCardTime
  creator?: EventCardCreator
  location: Location
  description?: string
  schedule?: EventCardTime & { recurrence?: string }
  jumpIn?: JumpIn
  remind?: RemindControl
  calendarHref?: string
  shareUrl: string
  onClose: () => void
}

type PlaceDetailModalProps = {
  title: string
  image: string | null
  creator?: EventCardCreator
  location: Location
  description?: string
  users?: number
  jumpIn?: JumpIn
  shareUrl: string
  onClose: () => void
}

/** Copies the share link; says so for a moment, or shows the link when the clipboard refuses. */
function CopyLink({ url }: { url: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  useEffect(() => {
    if (state !== 'copied') return
    const timer = setTimeout(() => setState('idle'), 2000)
    return () => clearTimeout(timer)
  }, [state])
  return (
    <>
      <button
        type="button"
        className={styles.iconButton}
        aria-label="Copy link"
        onClick={() => {
          if (!navigator.clipboard) {
            setState('failed')
            return
          }
          void navigator.clipboard.writeText(url).then(
            () => setState('copied'),
            () => setState('failed')
          )
        }}
      >
        <GeneralCopy size={20} aria-hidden="true" />
      </button>
      <span role="status" className={state === 'idle' ? styles.visuallyHidden : styles.copyNotice}>
        {state === 'copied' ? 'Copied!' : state === 'failed' ? `Copy this link: ${url}` : ''}
      </span>
    </>
  )
}

function LocationChip({ location }: { location: Location }) {
  const icon = location.world ? (
    <GeneralPublic size={18} aria-hidden="true" />
  ) : (
    <GeneralMapPin size={18} aria-hidden="true" />
  )
  if (!location.jumpIn) {
    return (
      <span className={styles.location}>
        {icon}
        {location.label}
      </span>
    )
  }
  return (
    <a
      className={styles.location}
      href={location.jumpIn.href}
      onClick={location.jumpIn.onJumpIn}
      aria-label={`Jump In – ${location.label}`}
    >
      {icon}
      {location.label}
    </a>
  )
}

function JumpInButton({ jumpIn }: { jumpIn: JumpIn }) {
  return (
    <Button href={jumpIn.href} onClick={jumpIn.onJumpIn}>
      Jump In
      <GeneralJumpIn size={20} aria-hidden="true" />
    </Button>
  )
}

function Hero({
  titleId,
  title,
  image,
  onClose,
  kicker,
  children
}: {
  titleId: string
  title: string
  image: string | null
  onClose: () => void
  /** The line above the title: the live badge or the start. */
  kicker?: ReactNode
  children: ReactNode
}) {
  return (
    <div className={styles.hero}>
      {image ? <img className={styles.heroImage} src={image} alt="" /> : null}
      <div className={styles.heroShade} aria-hidden="true" />
      <button type="button" className={styles.close} aria-label="Close" onClick={onClose}>
        <GeneralClose size={24} aria-hidden="true" />
      </button>
      <div className={styles.heroContent}>
        {kicker ? <div className={styles.kicker}>{kicker}</div> : null}
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        {children}
      </div>
    </div>
  )
}

/** An event's detail: hero with its actions, then what to expect and the schedule. */
function EventDetailModal(props: EventDetailModalProps) {
  const titleId = useId()
  const {
    live,
    when,
    creator,
    location,
    description,
    schedule,
    jumpIn,
    remind,
    calendarHref,
    shareUrl
  } = props
  return (
    <Dialog labelledBy={titleId} onClose={props.onClose}>
      <Hero
        titleId={titleId}
        title={props.title}
        image={props.image}
        onClose={props.onClose}
        kicker={
          live ? (
            <Badges live inline />
          ) : when ? (
            <time className={styles.when} dateTime={when.dateTime}>
              {when.label}
            </time>
          ) : null
        }
      >
        <div className={styles.meta}>
          {creator ? <CreatorRow creator={creator} prefix="By" /> : null}
          <LocationChip location={location} />
        </div>
        <div className={styles.actions}>
          {live && jumpIn ? <JumpInButton jumpIn={jumpIn} /> : null}
          {!live && remind ? (
            <Button
              variant={remind.active ? 'primary' : 'tertiary'}
              aria-pressed={remind.active}
              disabled={remind.pending}
              onClick={remind.onToggle}
            >
              <GeneralBell size={20} aria-hidden="true" />
              Remind me
            </Button>
          ) : null}
          {calendarHref ? (
            <a
              className={styles.iconButton}
              href={calendarHref}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Add to calendar"
            >
              <GeneralAddToCalendar size={20} aria-hidden="true" />
            </a>
          ) : null}
          <CopyLink url={shareUrl} />
        </div>
      </Hero>
      <div className={styles.body}>
        {description ? (
          <>
            <h3 className={styles.label}>What to expect</h3>
            <p className={styles.description}>{description}</p>
          </>
        ) : null}
        {description && schedule ? <hr className={styles.divider} /> : null}
        {schedule ? (
          <>
            <h3 className={styles.label}>Schedule</h3>
            <p className={styles.schedule}>
              <time dateTime={schedule.dateTime}>{schedule.label}</time>
            </p>
            {schedule.recurrence ? (
              <p className={styles.recurrence}>{schedule.recurrence}</p>
            ) : null}
          </>
        ) : null}
        {jumpIn ? (
          <div className={styles.bottomJump}>
            <JumpInButton jumpIn={jumpIn} />
          </div>
        ) : null}
      </div>
    </Dialog>
  )
}

/** A place's detail: hero with Jump In, then about, location and who is there now. */
function PlaceDetailModal(props: PlaceDetailModalProps) {
  const titleId = useId()
  const { creator, location, description, users, jumpIn, shareUrl } = props
  return (
    <Dialog labelledBy={titleId} onClose={props.onClose}>
      <Hero titleId={titleId} title={props.title} image={props.image} onClose={props.onClose}>
        {creator ? (
          <div className={styles.meta}>
            <CreatorRow creator={creator} prefix="By" />
          </div>
        ) : null}
        <div className={styles.actions}>
          {jumpIn ? <JumpInButton jumpIn={jumpIn} /> : null}
          <CopyLink url={shareUrl} />
        </div>
      </Hero>
      <div className={styles.body}>
        {description ? (
          <>
            <h3 className={styles.label}>About this place</h3>
            <p className={styles.description}>{description}</p>
            <hr className={styles.divider} />
          </>
        ) : null}
        <h3 className={styles.label}>{location.world ? 'World' : 'Location'}</h3>
        <p className={styles.metaText}>
          {location.world ? (
            <GeneralPublic size={18} aria-hidden="true" />
          ) : (
            <GeneralMapPin size={18} aria-hidden="true" />
          )}
          {location.label}
        </p>
        {users ? (
          <>
            <hr className={styles.divider} />
            <p className={styles.metaText}>
              <GeneralPlayers size={18} aria-hidden="true" />
              {users === 1 ? '1 person here now' : `${users} people here now`}
            </p>
          </>
        ) : null}
      </div>
    </Dialog>
  )
}

export { EventDetailModal, PlaceDetailModal }
export type { EventDetailModalProps, PlaceDetailModalProps }
