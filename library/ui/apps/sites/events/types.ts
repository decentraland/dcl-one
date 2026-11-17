import type { MouseEvent } from 'react'

/**
 * The data the /events page hands its sections. `cards.tsx` maps it onto the public cards: PlaceCard
 * (live variant) for Live Now, EventCard (small) for Upcoming and the phone's day list, EventCard (day)
 * for the day columns.
 */
type EventCardCreator = {
  name: string
  href?: string
  avatarUrl?: string
  /** Shows the Decentraland logo as the avatar, as the source does for Genesis Plaza. */
  foundation?: boolean
}

type EventCardData = {
  id: string
  title: string
  image: string | null
  /** The event or place detail; the whole card follows it. */
  href: string
  /** Runs before `href` is followed, so the page can open its detail in place. */
  onOpen?: (event: MouseEvent<HTMLAnchorElement>) => void
  users?: number
  /** Names the user count for assistive technology, e.g. "12 online". */
  usersLabel?: string
  creator?: EventCardCreator
}

/** Remind me for one event: signed by the member, toggled optimistically by the page. */
type RemindControl = {
  active: boolean
  pending: boolean
  onToggle: () => void
}

/** A start time already formatted for the viewer, and its machine-readable instant. */
type EventCardTime = { label: string; dateTime?: string }

type JumpIn = {
  href: string
  onJumpIn?: (event: MouseEvent<HTMLAnchorElement>) => void
}

/** Live Now: a live event, or a busy place (`live: false`). */
type LiveEventCardProps = EventCardData & {
  live: boolean
  jumpIn?: JumpIn
}

/** Upcoming: an event starting within the day. */
type UpcomingEventCardProps = EventCardData & {
  time: EventCardTime
  remind?: RemindControl
  calendarHref?: string
}

/**
 * All Events: one event in a day column; live ones show their badges instead of a time. The phone's
 * day list shows a live one's JUMP IN too.
 */
type ExperienceCardProps = EventCardData & {
  live: boolean
  time: EventCardTime
  remind?: RemindControl
  calendarHref?: string
  /** Read only by the phone's day list (a small card); a day column shows no JUMP IN. */
  jumpIn?: JumpIn
}

export type {
  EventCardCreator,
  EventCardData,
  EventCardTime,
  ExperienceCardProps,
  JumpIn,
  LiveEventCardProps,
  RemindControl,
  UpcomingEventCardProps
}
