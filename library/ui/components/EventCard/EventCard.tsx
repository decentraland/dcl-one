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
   * `small` is the design system's `size=small` card (an Upcoming row, the phone's day list): the text
   * on the left, a square cover on the right. `day` puts the cover above (a day column), and leaves it
   * out when there is no image and the event is not live.
   */
  variant?: 'small' | 'day'
  title: string
  /** The event's page. The whole card is the title link's target. */
  href: string
  /** Runs on a click of the title link, before `href` is followed, e.g. to open the event in place. */
  onOpen?: (event: MouseEvent<HTMLAnchorElement>) => void
  image?: string | null
  /** Loads the cover eagerly, for the first card of a page. */
  priority?: boolean
  /**
   * A small card credits the host as plain text: hover folds the row away, so a link there could not
   * be reached with a pointer. The day card links `profileHref` and shows the face.
   */
  creator?: EventCardCreator
  /** The word before the host's name. */
  byLabel?: string
  /**
   * The start time. In a day card, hover or keyboard focus swaps it for the actions; a small card keeps
   * it beside them, unless the event is live.
   */
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
  /**
   * The caller's actions, usually `EventCardAction` and `EventCardActionLink`; a live small card takes
   * the public `JumpInCTA` (size `sm`) and a share action.
   */
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
 * focus inside the card lifts it and reveals the caller's actions, which fade and slide in: a day card
 * swaps the start time for them; a small card folds the host away (and a live one its start time too)
 * and opens a row of actions under the time. On a touch screen or a narrow one the actions stay shown.
 * The title is the card's link (its hit area covers the card); the host link and the actions stay
 * separate controls above it.
 */
export function EventCard({
  variant = 'small',
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
  const heading = (
    <Heading className={variant === 'small' ? styles.smallTitle : styles.title}>
      <Link className={styles.link} href={href} onClick={onOpen}>
        {title}
      </Link>
    </Heading>
  )
  const badges = hasBadges ? (
    <Badges live={live} liveLabel={liveLabel} players={players} playersLabel={playersLabel} />
  ) : null
  const status =
    announcement !== undefined ? (
      <span role="status" className={styles.visuallyHidden}>
        {announcement}
      </span>
    ) : null
  if (variant === 'small')
    return (
      <article
        className={[
          styles.card,
          styles.small,
          live ? styles.isLive : undefined,
          hasActions ? styles.hasActions : undefined
        ]
          .filter(Boolean)
          .join(' ')}
      >
        <div className={styles.body}>
          {heading}
          {creator ? (
            <div className={[styles.reveal, styles.hideOnHover].join(' ')}>
              <div className={styles.revealInner}>
                <p className={styles.smallBy}>
                  {byLabel} {creator.name}
                </p>
              </div>
            </div>
          ) : null}
          {time ? (
            <div className={[styles.reveal, live ? styles.hideOnHover : undefined].filter(Boolean).join(' ')}>
              <div className={[styles.revealInner, styles.timeRow].join(' ')}>
                <p className={styles.smallTime}>
                  <GeneralTime size={20} aria-hidden="true" />
                  <time dateTime={time.dateTime}>{time.label}</time>
                </p>
              </div>
            </div>
          ) : null}
          {hasActions ? (
            <div className={[styles.reveal, styles.showOnHover].join(' ')}>
              <div className={[styles.revealInner, styles.actionsRow].join(' ')}>
                <div className={styles.actions}>{actions}</div>
              </div>
            </div>
          ) : null}
          {status}
        </div>
        <div className={styles.media}>
          <CardImage className={styles.cover} src={image} alt="" priority={priority} />
          {badges}
        </div>
      </article>
    )
  const showMedia = Boolean(image) || hasBadges
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
          {image ? <CardImage className={styles.cover} src={image} alt="" priority={priority} /> : null}
          {badges}
        </div>
      ) : null}
      <div className={styles.body}>
        {heading}
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
        {status}
      </div>
    </article>
  )
}

type BadgesProps = Pick<EventCardProps, 'live' | 'liveLabel' | 'players' | 'playersLabel'>

/** LIVE and the count of people over the cover. A small card shows LIVE as its glyph only. */
function Badges({ live, liveLabel, players, playersLabel }: BadgesProps) {
  return (
    <div className={styles.badges}>
      {live ? (
        <span className={styles.live}>
          <GeneralLive className={styles.liveIcon} aria-hidden="true" />
          <span className={styles.liveLabel}>{liveLabel}</span>
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
  /** The glyph: 20px in a day card, 24px in a small one. */
  icon: ReactNode
  /**
   * Visible text after the glyph, e.g. "Remind me", for a day card; a small card's actions are
   * icon-only. `aria-label` names the action either way.
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
