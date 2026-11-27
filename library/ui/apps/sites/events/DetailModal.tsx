import { useEffect, useId, useState, type ReactNode } from 'react'
import {
  GeneralAddToCalendar,
  GeneralBell,
  GeneralCopy,
  GeneralJumpIn,
  GeneralLive,
  GeneralMapPin,
  GeneralPlayers,
  GeneralPublic
} from '../../../foundations/icons'
import { Button } from '../../../components/Button/Button'
import { CloseButton } from '../../../components/CloseButton/CloseButton'
import { EventCardHost } from '../../../components/EventCard/EventCard'
import { hostOf } from './cards'
import type { EventCardCreator, EventCardTime, JumpIn, RemindControl } from './types'
import { Dialog } from './Dialog'
import { useEventsCopy } from './locale'
import styles from './DetailModal.module.css'

const reminderActionSize = { size: 'md', viewport: 'responsive' } as const
/** Calendar and Copy beside the large Jump In, when no reminder sets the row's size. */
const largeActionSize = { size: 'lg' } as const

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
  shareUrl?: string
  actions?: ReactNode
  feedback?: ReactNode
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
function CopyLink({ url, responsive = false }: { url: string; responsive?: boolean }) {
  const { text } = useEventsCopy()
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  useEffect(() => {
    if (state !== 'copied') return
    const timer = setTimeout(() => setState('idle'), 2000)
    return () => clearTimeout(timer)
  }, [state])
  function copy() {
    if (!navigator.clipboard) {
      setState('failed')
      return
    }
    void navigator.clipboard.writeText(url).then(
      () => setState('copied'),
      () => setState('failed')
    )
  }
  return (
    <>
      <Button
        {...(responsive ? reminderActionSize : largeActionSize)}
        variant="secondaryLight"
        iconOnly
        aria-label={text('Copy link')}
        startIcon={<GeneralCopy aria-hidden="true" />}
        onClick={copy}
      />
      <span role="status" className={state === 'idle' ? styles.visuallyHidden : styles.copyNotice}>
        {state === 'copied'
          ? text('Copied!')
          : state === 'failed'
            ? text('Copy this link: {url}', { url })
            : ''}
      </span>
    </>
  )
}

function CalendarLink({ href, responsive }: { href: string; responsive: boolean }) {
  const { text } = useEventsCopy()
  return (
    <Button
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={text('Add to calendar')}
      {...(responsive ? reminderActionSize : largeActionSize)}
      variant="secondaryLight"
      iconOnly
      startIcon={<GeneralAddToCalendar aria-hidden="true" />}
    />
  )
}

function LocationChip({ location }: { location: Location }) {
  const { text } = useEventsCopy()
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
      aria-label={text('Jump In – {location}', { location: location.label })}
    >
      {icon}
      {location.label}
    </a>
  )
}

function JumpInButton({ jumpIn }: { jumpIn: JumpIn }) {
  const { text } = useEventsCopy()
  return (
    <Button
      size="lg"
      href={jumpIn.href}
      onClick={jumpIn.onJumpIn}
      trailingIcon={<GeneralJumpIn size={20} aria-hidden="true" />}
    >
      {text('Jump In')}
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
  const { text } = useEventsCopy()
  return (
    <div className={styles.hero}>
      {image ? <img className={styles.heroImage} src={image} alt="" /> : null}
      <div className={styles.heroShade} aria-hidden="true" />
      <CloseButton
        variant="filled"
        className={styles.close}
        aria-label={text('Close')}
        onClick={onClose}
      />
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
  const { text } = useEventsCopy()
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
  const reminder = live ? undefined : remind
  return (
    <Dialog labelledBy={titleId} onClose={props.onClose}>
      <Hero
        titleId={titleId}
        title={props.title}
        image={props.image}
        onClose={props.onClose}
        kicker={
          live ? (
            <span className={styles.liveBadge}>
              <GeneralLive size={16} aria-hidden="true" />
              {text('LIVE')}
            </span>
          ) : when ? (
            <time className={styles.when} dateTime={when.dateTime}>
              {when.label}
            </time>
          ) : null
        }
      >
        <div className={styles.meta}>
          {creator ? <EventCardHost creator={hostOf(creator)} byLabel={text('By')} /> : null}
          <LocationChip location={location} />
        </div>
        <div className={styles.actions}>
          {live && jumpIn ? <JumpInButton jumpIn={jumpIn} /> : null}
          {reminder ? (
            <Button
              {...reminderActionSize}
              variant={reminder.active ? 'primary' : 'secondaryLight'}
              startIcon={<GeneralBell aria-hidden="true" />}
              aria-pressed={reminder.active}
              aria-disabled={reminder.pending}
              onClick={reminder.onToggle}
            >
              {text('Remind me')}
            </Button>
          ) : null}
          {calendarHref ? (
            <CalendarLink href={calendarHref} responsive={Boolean(reminder)} />
          ) : null}
          {shareUrl ? <CopyLink url={shareUrl} responsive={Boolean(reminder)} /> : null}
          {props.actions}
        </div>
      </Hero>
      <div className={styles.body}>
        {props.feedback}
        {description ? (
          <>
            <h3 className={styles.label}>{text('What to expect')}</h3>
            <p className={styles.description}>{description}</p>
          </>
        ) : null}
        {description && schedule ? <hr className={styles.divider} /> : null}
        {schedule ? (
          <>
            <h3 className={styles.label}>{text('Schedule')}</h3>
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
  const { text } = useEventsCopy()
  const titleId = useId()
  const { creator, location, description, users, jumpIn, shareUrl } = props
  return (
    <Dialog labelledBy={titleId} onClose={props.onClose}>
      <Hero titleId={titleId} title={props.title} image={props.image} onClose={props.onClose}>
        {creator ? (
          <div className={styles.meta}>
            <EventCardHost creator={hostOf(creator)} byLabel={text('By')} />
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
            <h3 className={styles.label}>{text('About this place')}</h3>
            <p className={styles.description}>{description}</p>
            <hr className={styles.divider} />
          </>
        ) : null}
        <h3 className={styles.label}>{location.world ? text('World') : text('Location')}</h3>
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
              {text(users === 1 ? '{count} person here now' : '{count} people here now', {
                count: users
              })}
            </p>
          </>
        ) : null}
      </div>
    </Dialog>
  )
}

export { EventDetailModal, PlaceDetailModal }
export type { EventDetailModalProps, PlaceDetailModalProps }
