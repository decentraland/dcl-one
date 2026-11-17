import { useState } from 'react'
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
  GeneralBell,
  GeneralCopy,
  GeneralReminderOff,
  GeneralReminderOn,
  MenusShare
} from '../../../foundations/icons'
import dclLogo from '../assets/dcl-logo.svg'
import type {
  EventCardCreator,
  ExperienceCardProps,
  JumpIn,
  LiveEventCardProps,
  RemindControl,
  UpcomingEventCardProps
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
      jumpLabel="Jump In"
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
  const [status, setStatus] = useState('')
  const copy = async () => {
    try {
      if (!navigator.clipboard) throw new Error('Clipboard unavailable')
      await navigator.clipboard.writeText(new URL(href, window.location.href).href)
      setStatus('Link copied.')
    } catch {
      setStatus("Couldn't copy the link. Try again.")
    }
  }
  return { status, copy }
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
  const small = size === 'small'
  const glyph = small ? 24 : 20
  const Reminder = small ? (remind?.active ? GeneralReminderOn : GeneralReminderOff) : GeneralBell
  const Share = small ? MenusShare : GeneralCopy
  const share = (
    <EventCardAction
      aria-label={`Copy link: ${title}`}
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
            href={jumpIn.href}
            onClick={jumpIn.onJumpIn}
            aria-label={`Jump in: ${title}`}
          />
        ) : null}
        {share}
      </>
    )
  return (
    <>
      {remind ? (
        <EventCardAction
          aria-label={`Remind me: ${title}`}
          aria-pressed={remind.active}
          disabled={remind.pending}
          icon={<Reminder size={glyph} aria-hidden="true" />}
          label={small ? undefined : 'Remind me'}
          onClick={remind.onToggle}
        />
      ) : null}
      {calendarHref ? (
        <EventCardActionLink
          aria-label={`Add to calendar: ${title}`}
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
}: UpcomingEventCardProps & { live?: boolean; jumpIn?: JumpIn }) {
  const { status, copy } = useCopyLink(href)
  return (
    <EventCard
      variant="small"
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
    />
  )
}

/** A card in an All Events day column: a live event shows its badges, a later one its time and actions. */
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
  calendarHref
}: ExperienceCardProps) {
  const { status, copy } = useCopyLink(href)
  return (
    <EventCard
      variant="day"
      title={title}
      href={href}
      onOpen={onOpen}
      image={image}
      creator={creatorOf(creator)}
      byLabel="By"
      live={live}
      players={live ? users : undefined}
      playersLabel={live ? usersLabel : undefined}
      time={live ? undefined : time}
      actions={
        live ? undefined : (
          <EventActions
            size="day"
            title={title}
            remind={remind}
            calendarHref={calendarHref}
            onCopy={() => void copy()}
          />
        )
      }
      announcement={live ? undefined : status}
    />
  )
}

export { DayCard, LiveNowCard, UpcomingCard }
