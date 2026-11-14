import type { MouseEvent } from 'react'

/**
 * The card contract the /events page renders through. It follows `WhatsOnCardData`
 * (`../../home/WhatsOn.tsx`) and adds only what events need. The shared EventCard and PlaceCard
 * will be wired to these props inside the slot files, so the page does not change when they land.
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
  /** Loads the first card's image eagerly: it is the rail's largest paint. */
  eager?: boolean
}

/** Upcoming: an event starting within the day. */
type UpcomingEventCardProps = EventCardData & {
  time: EventCardTime
  remind?: RemindControl
  calendarHref?: string
}

/** All Events: one event in a day column; live ones show their badges instead of a time. */
type ExperienceCardProps = EventCardData & {
  live: boolean
  time: EventCardTime
  remind?: RemindControl
  calendarHref?: string
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
