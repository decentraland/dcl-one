import { useState } from 'react'
import {
  EventCard,
  EventCardAction,
  EventCardActionLink,
  type EventCardCreator as PublicCreator
} from '../../../components/EventCard/EventCard'
import { PlaceCard } from '../../../components/PlaceCard/PlaceCard'
import { GeneralAddToCalendar, GeneralBell, GeneralCopy } from '../../../foundations/icons'
import dclLogo from '../assets/dcl-logo.svg'
import type {
  EventCardCreator,
  ExperienceCardProps,
  LiveEventCardProps,
  RemindControl,
  UpcomingEventCardProps
} from './types'

/** The page's creator as the public cards take it; Genesis Plaza's host shows the Decentraland logo. */
function creatorOf(creator: EventCardCreator | undefined): PublicCreator | undefined {
  if (!creator) return undefined
  return {
    name: creator.name,
    avatar: creator.avatarUrl ?? (creator.foundation ? dclLogo : undefined),
    profileHref: creator.href
  }
}

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
  jumpIn
}: LiveEventCardProps) {
  return (
    <PlaceCard
      variant="live"
      title={title}
      href={href}
      onOpen={onOpen}
      image={image ?? undefined}
      // A missing count is unknown and shows none; a known zero shows 0.
      players={users ?? 0}
      showPlayers={users !== undefined}
      playersLabel={usersLabel}
      live={live}
      liveEventName={live ? title : undefined}
      creator={creatorOf(creator)}
      jumpHref={jumpIn?.href}
      jumpLabel="Jump In"
      onJumpIn={jumpIn?.onJumpIn}
    />
  )
}

type ActionProps = { title: string; href: string; remind?: RemindControl; calendarHref?: string }

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

/** Remind me, Add to calendar and Copy link, as the public EventCard's actions. */
function EventActions({
  title,
  remind,
  calendarHref,
  onCopy
}: Omit<ActionProps, 'href'> & { onCopy: () => void }) {
  return (
    <>
      {remind ? (
        <EventCardAction
          aria-label={`Remind me: ${title}`}
          aria-pressed={remind.active}
          disabled={remind.pending}
          icon={<GeneralBell size={20} aria-hidden="true" />}
          onClick={remind.onToggle}
        />
      ) : null}
      {calendarHref ? (
        <EventCardActionLink
          aria-label={`Add to calendar: ${title}`}
          href={calendarHref}
          target="_blank"
          rel="noopener noreferrer"
          icon={<GeneralAddToCalendar size={20} aria-hidden="true" />}
        />
      ) : null}
      <EventCardAction
        aria-label={`Copy link: ${title}`}
        icon={<GeneralCopy size={20} aria-hidden="true" />}
        onClick={onCopy}
      />
    </>
  )
}

/** Upcoming, and the phone's single-day list: the public EventCard's compact variant. */
function UpcomingCard({
  title,
  image,
  href,
  onOpen,
  creator,
  time,
  remind,
  calendarHref
}: UpcomingEventCardProps) {
  const { status, copy } = useCopyLink(href)
  return (
    <EventCard
      variant="compact"
      title={title}
      href={href}
      onOpen={onOpen}
      image={image}
      creator={creatorOf(creator)}
      time={time}
      actions={
        <EventActions
          title={title}
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
