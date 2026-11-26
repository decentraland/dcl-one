import { useEffect, useRef, useState } from 'react'
import {
  EventCard,
  EventCardAction,
  EventCardActionLink,
  type EventCardCreator as PublicCreator
} from '../../../components/EventCard/EventCard'
import { JumpInCTA } from '../../../components/JumpInCTA/JumpInCTA'
import { PlaceCard } from '../../../components/PlaceCard/PlaceCard'
import {
  GeneralAddToCalendar,
  GeneralCopy,
  GeneralReminderOff,
  GeneralReminderOn,
  MenusShare
} from '../../../foundations/icons'
import dclLogo from '../assets/dcl-logo.svg'
import { useEventsCopy } from './locale'
import styles from './cards.module.css'
import type {
  DayListCardProps,
  EventCardCreator,
  ExperienceCardProps,
  JumpIn,
  LiveEventCardProps,
  MyEventCardProps,
  RemindControl
} from './types'

/** The page's host as the public cards take it; Genesis Plaza's host shows the Decentraland logo. */
export function hostOf(creator: EventCardCreator): PublicCreator {
  return {
    name: creator.name,
    avatar: creator.avatarUrl ?? (creator.foundation ? dclLogo : undefined),
    profileHref: creator.href
  }
}

const creatorOf = (creator: EventCardCreator | undefined) => (creator ? hostOf(creator) : undefined)

/** Live Now: a live event or a busy place, as the public PlaceCard's Live Now variant. */
function LiveNowCard({
  title,
  image,
  href,
  onOpen,
  users,
  usersLabel,
  creator,
  live,
  jumpIn,
  priority = false
}: LiveEventCardProps & { priority?: boolean }) {
  const { text } = useEventsCopy()
  return (
    <PlaceCard
      variant="live"
      title={title}
      href={href}
      onOpen={onOpen}
      image={image ?? undefined}
      priority={priority}
      // A missing count is unknown and shows none; a known zero shows 0.
      players={users ?? 0}
      showPlayers={users !== undefined}
      playersLabel={usersLabel}
      live={live}
      creator={creatorOf(creator)}
      jumpHref={jumpIn?.href}
      jumpLabel={text('Jump In')}
      liveLabel={text('LIVE')}
      byLabel={text('By')}
      onJumpIn={jumpIn?.onJumpIn}
    />
  )
}

type ActionProps = { title: string; remind?: RemindControl; calendarHref?: string }

/**
 * Copy link's outcome, for the card to announce. The card renders the status region whenever the
 * action exists, so the first result is read too.
 */
function useCopyLink(href: string) {
  const { text } = useEventsCopy()
  const [status, setStatus] = useState<'idle' | 'copied' | 'failed'>('idle')
  const attempt = useRef(0)
  useEffect(() => {
    if (status !== 'copied') return
    const timer = setTimeout(() => setStatus('idle'), 2000)
    return () => clearTimeout(timer)
  }, [status])

  const copy = async () => {
    const current = ++attempt.current
    setStatus('idle')
    let outcome: 'copied' | 'failed'
    try {
      await navigator.clipboard.writeText(new URL(href, window.location.href).href)
      outcome = 'copied'
    } catch {
      outcome = 'failed'
    }
    if (current === attempt.current) setStatus(outcome)
  }
  const message =
    status === 'copied'
      ? text('Copied')
      : status === 'failed'
        ? text("Couldn't copy the link. Try again.")
        : ''
  return { status: message, copy }
}

/**
 * The public EventCard's actions. A day card: Remind me (with its label), Add to calendar and Copy link
 * at 20px. A small card: the same as icon-only 24px glyphs, or for a live event JUMP IN (when it has
 * somewhere to go) and Copy link.
 */
function EventActions({
  title,
  size,
  live = false,
  jumpIn,
  remind,
  calendarHref,
  onCopy
}: ActionProps & {
  size: 'small' | 'day'
  live?: boolean
  jumpIn?: JumpIn
  onCopy: () => void
}) {
  const { text } = useEventsCopy()
  const small = size === 'small'
  const glyph = small ? 24 : 20
  const [requestedReminder, setRequestedReminder] = useState(false)
  const Reminder = remind?.active ? GeneralReminderOn : GeneralReminderOff
  const Share = small ? MenusShare : GeneralCopy
  const share = (
    <EventCardAction
      aria-label={text('{action}: {title}', { action: text('Copy link'), title })}
      icon={<Share size={glyph} aria-hidden="true" />}
      onClick={onCopy}
    />
  )
  if (live)
    return (
      <>
        {jumpIn ? (
          <JumpInCTA
            size="sm"
            label={text('Jump in')}
            href={jumpIn.href}
            onClick={jumpIn.onJumpIn}
            aria-label={text('{action}: {title}', { action: text('Jump in'), title })}
          />
        ) : null}
        {share}
      </>
    )
  return (
    <>
      {remind ? (
        <EventCardAction
          aria-label={text('{action}: {title}', { action: text('Remind me'), title })}
          aria-pressed={remind.active}
          disabled={remind.pending}
          icon={
            <Reminder
              size={glyph}
              className={requestedReminder && remind.active ? styles.reminderShake : undefined}
              aria-hidden="true"
            />
          }
          label={small ? undefined : text('Remind me')}
          onClick={() => {
            setRequestedReminder(!remind.active)
            remind.onToggle()
          }}
        />
      ) : null}
      {calendarHref ? (
        <EventCardActionLink
          aria-label={text('{action}: {title}', { action: text('Add to calendar'), title })}
          href={calendarHref}
          target="_blank"
          rel="noopener noreferrer"
          icon={<GeneralAddToCalendar size={glyph} aria-hidden="true" />}
        />
      ) : null}
      {share}
    </>
  )
}

/**
 * Upcoming, and the phone's single-day list: the public EventCard's small variant. A live event (the
 * phone list) shows its badges and JUMP IN instead of Remind me.
 */
function UpcomingCard({
  title,
  image,
  href,
  onOpen,
  creator,
  time,
  remind,
  calendarHref,
  live = false,
  users,
  usersLabel,
  jumpIn
}: DayListCardProps) {
  const { text } = useEventsCopy()
  const { status, copy } = useCopyLink(href)
  return (
    <EventCard
      variant="small"
      liveLabel={text('LIVE')}
      byLabel={text('by')}
      title={title}
      href={href}
      onOpen={onOpen}
      image={image}
      creator={creatorOf(creator)}
      time={time}
      live={live}
      players={live ? users : undefined}
      playersLabel={live ? usersLabel : undefined}
      actions={
        <EventActions
          size="small"
          title={title}
          live={live}
          jumpIn={jumpIn}
          remind={remind}
          calendarHref={calendarHref}
          onCopy={() => void copy()}
        />
      }
      announcement={status}
      showAnnouncement
    />
  )
}

/** A card in an All Events day column, with actions matched to the event state. */
function DayCard({
  title,
  image,
  href,
  onOpen,
  users,
  usersLabel,
  creator,
  live,
  time,
  remind,
  calendarHref,
  jumpIn
}: ExperienceCardProps) {
  const { text } = useEventsCopy()
  const { status, copy } = useCopyLink(href)
  return (
    <EventCard
      variant="day"
      title={title}
      href={href}
      onOpen={onOpen}
      image={image}
      creator={creatorOf(creator)}
      byLabel={text('By')}
      liveLabel={text('LIVE')}
      live={live}
      players={live ? users : undefined}
      playersLabel={live ? usersLabel : undefined}
      time={live ? undefined : time}
      actions={
        <EventActions
          size="day"
          title={title}
          live={live}
          jumpIn={jumpIn}
          remind={remind}
          calendarHref={calendarHref}
          onCopy={() => void copy()}
        />
      }
      announcement={status}
      showAnnouncement
    />
  )
}

const STATUS_LABEL = { pending: 'Pending', approved: 'Approved', rejected: 'Rejected' } as const

/**
 * My Events: the day card without actions, opening the event's editor. Its review status sits over the
 * cover, and the cover of an event still in review is dimmed.
 */
function MyEventCard({ title, image, href, creator, time, status }: MyEventCardProps) {
  const { text } = useEventsCopy()
  return (
    <div className={styles.mine} data-status={status} data-cover={image ? '' : undefined}>
      <span className={styles.status}>{text(STATUS_LABEL[status])}</span>
      <EventCard
        variant="day"
        title={title}
        href={href}
        image={image}
        creator={creatorOf(creator)}
        byLabel={text('By')}
        time={time}
        dimmed={status === 'pending'}
      />
    </div>
  )
}

export { DayCard, LiveNowCard, MyEventCard, UpcomingCard }
