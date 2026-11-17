import type {
  AnchorHTMLAttributes,
  ButtonHTMLAttributes,
  ComponentType,
  MouseEvent,
  ReactNode
} from 'react'
import { GeneralLive, GeneralPlayers, GeneralTime } from '../../foundations/icons/general'
import { CardImage } from '../CardImage/CardImage'
import styles from './EventCard.module.css'

/** The router's link, passed in so the card opens its event under any router. */
export type EventCardLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }
export type EventCardLinkComponent = ComponentType<EventCardLinkProps>

/** The credited host. `profileHref` makes the name a link to their profile. */
export type EventCardCreator = {
  name: string
  /** The host's face; the name's initial without it. */
  avatar?: string
  profileHref?: string
}

/** A start time already formatted for the viewer, and its machine-readable instant. */
export type EventCardTime = { label: string; dateTime?: string }

export type EventCardProps = {
  /**
   * `compact` puts the cover beside the text (an Upcoming row); `day` puts it above (a day column),
   * and leaves it out when there is no image and the event is not live.
   */
  variant?: 'compact' | 'day'
  title: string
  /** The event's page. The whole card is the title link's target. */
  href: string
  /** Runs on a click of the title link, before `href` is followed, e.g. to open the event in place. */
  onOpen?: (event: MouseEvent<HTMLAnchorElement>) => void
  image?: string | null
  /** Loads the cover eagerly, for the first card of a page. */
  priority?: boolean
  creator?: EventCardCreator
  /** The word before the host's name. */
  byLabel?: string
  /** The start time. Hover or keyboard focus swaps it for the actions. */
  time?: EventCardTime
  live?: boolean
  /** The LIVE badge's text. */
  liveLabel?: string
  /**
   * How many people are in the event, shown only when given: a missing count is unknown, not zero.
   */
  players?: number
  /** Names the count for assistive technology, e.g. "12 online". */
  playersLabel?: string
  /** The caller's actions, usually `EventCardAction` and `EventCardActionLink`. */
  actions?: ReactNode
  /** Announced to assistive technology, not shown: the outcome of an action, e.g. "Link copied.". */
  announcement?: string
  /** Renders `href` (a router link); a plain anchor without it. */
  linkComponent?: EventCardLinkComponent
  /** The title's heading level. */
  headingLevel?: 2 | 3 | 4 | 5 | 6
}

function Anchor(props: EventCardLinkProps) {
  return <a {...props} />
}

/**
 * An event in a list: its cover, title, host and start time. On a device that hovers, hover or keyboard
 * focus inside the card lifts it and swaps the start time for the caller's actions, which fade and slide
 * in; on a touch screen or a narrow one the actions stay shown under the time. The title is the card's
 * link (its hit area covers the card); the host link and the actions stay separate controls above it.
 */
export function EventCard({
  variant = 'compact',
  title,
  href,
  onOpen,
  image,
  priority = false,
  creator,
  byLabel = 'by',
  time,
  live = false,
  liveLabel = 'LIVE',
  players,
  playersLabel,
  actions,
  announcement,
  linkComponent: Link = Anchor,
  headingLevel = 3
}: EventCardProps) {
  const Heading = `h${headingLevel}` as const
  const hasActions = actions !== undefined && actions !== null && actions !== false
  const hasBadges = live || players !== undefined
  const showMedia = variant === 'compact' || Boolean(image) || hasBadges
  return (
    <article
      className={[
        styles.card,
        styles[variant],
        live ? styles.isLive : undefined,
        hasActions ? styles.hasActions : undefined
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {showMedia ? (
        <div className={styles.media}>
          {image || variant === 'compact' ? (
            <CardImage className={styles.cover} src={image} alt="" priority={priority} />
          ) : null}
          {hasBadges ? (
            <div className={styles.badges}>
              {live ? (
                <span className={styles.live}>
                  <GeneralLive size={16} aria-hidden="true" />
                  {liveLabel}
                </span>
              ) : null}
              {players !== undefined ? (
                <span className={styles.players}>
                  <span className={styles.dot} aria-hidden="true" />
                  <GeneralPlayers size={14} aria-hidden="true" />
                  <span aria-hidden={playersLabel ? true : undefined}>{players}</span>
                  {playersLabel ? <span className={styles.visuallyHidden}>{playersLabel}</span> : null}
                </span>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
      <div className={styles.body}>
        <Heading className={styles.title}>
          <Link className={styles.link} href={href} onClick={onOpen}>
            {title}
          </Link>
        </Heading>
        {creator ? <EventCardHost creator={creator} byLabel={byLabel} /> : null}
        {time || hasActions ? (
          <div className={styles.footer}>
            {time ? (
              <p className={styles.time}>
                <GeneralTime size={16} aria-hidden="true" />
                <time dateTime={time.dateTime}>{time.label}</time>
              </p>
            ) : null}
            {hasActions ? <div className={styles.actions}>{actions}</div> : null}
          </div>
        ) : null}
        {announcement !== undefined ? (
          <span role="status" className={styles.visuallyHidden}>
            {announcement}
          </span>
        ) : null}
      </div>
    </article>
  )
}

export type EventCardHostProps = { creator: EventCardCreator; byLabel?: string }

/** "by <host>" with the host's face or initial; also usable outside a card, e.g. in an event's detail. */
export function EventCardHost({ creator, byLabel = 'by' }: EventCardHostProps) {
  return (
    <div className={styles.creator}>
      {creator.avatar ? (
        <img className={styles.avatar} src={creator.avatar} alt="" loading="lazy" />
      ) : (
        <span className={styles.avatar} aria-hidden="true">
          {creator.name.charAt(0).toUpperCase()}
        </span>
      )}
      <p className={styles.by}>
        {byLabel}{' '}
        {creator.profileHref ? (
          <a className={styles.name} href={creator.profileHref}>
            {creator.name}
          </a>
        ) : (
          <span className={styles.name}>{creator.name}</span>
        )}
      </p>
    </div>
  )
}

export type EventCardActionProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> & {
  /** The action has no visible text, so it needs an accessible name. */
  'aria-label': string
  /** The 20px glyph. */
  icon: ReactNode
  /**
   * Visible text after the glyph, e.g. "Remind me". A compact card narrower than 400px shows the glyph
   * only; `aria-label` names the action either way.
   */
  label?: string
}

/**
 * An action of an EventCard, named by `aria-label`. A toggle (Remind me) passes `aria-pressed`, which
 * also tints its glyph.
 */
export function EventCardAction({ icon, label, className, type = 'button', ...rest }: EventCardActionProps) {
  return (
    <button
      {...rest}
      type={type}
      className={[styles.action, label ? styles.labelled : undefined, className].filter(Boolean).join(' ')}
    >
      {icon}
      {label ? <span className={styles.actionLabel}>{label}</span> : null}
    </button>
  )
}

export type EventCardActionLinkProps = Omit<
  AnchorHTMLAttributes<HTMLAnchorElement>,
  'children'
> & {
  'aria-label': string
  href: string
  icon: ReactNode
}

/** An icon-only action of an EventCard that goes somewhere, e.g. Add to calendar. */
export function EventCardActionLink({ icon, className, ...rest }: EventCardActionLinkProps) {
  return (
    <a {...rest} className={[styles.action, className].filter(Boolean).join(' ')}>
      {icon}
    </a>
  )
}
