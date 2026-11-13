import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
  type ReactNode
} from 'react'
import { JumpInIcon } from '../../../components/JumpInCTA/JumpInCTA'
import creatorPlaceholder from '../assets/jump/card-creator-placeholder.webp'
import decentralandLogo from '../assets/dcl-logo.svg'
import eventPlaceholder from '../assets/jump/card-events-placeholder.webp'
import placePlaceholder from '../assets/jump/card-places-placeholder.webp'
import { Button } from '../shared/Button/Button'
import {
  AccessTimeIcon,
  BellIcon,
  CircleIcon,
  LiveEventIcon,
  PersonIcon,
  PlaceIcon,
  PublicIcon
} from './JumpIcons'
import styles from './JumpCard.module.css'

type JumpCardBadge =
  | { kind: 'live'; count: number }
  | { kind: 'attendees'; count: number }
  | { kind: 'players'; count: number }

type JumpCardProps = {
  kind: 'place' | 'event'
  title: string
  /** `foundation` shows the Decentraland mark when no avatar is known; `color` is the disc behind the avatar. */
  creator: { name: string; avatar?: string; href?: string; foundation?: boolean; color?: string }
  image?: string
  description?: string
  /** The World name, or the parcel as "x, y". */
  location: { world?: string; parcel: string }
  /** Event start, already formatted, or the ended notice. */
  when?: { label: string; ended: boolean; dateTime?: string }
  badge?: JumpCardBadge
  /** Where Jump In travels to; omitted when the card shows no Jump In. */
  jumpInHref?: string
  /** Runs on Jump In before the link navigates, for launches the link alone cannot complete. */
  onJumpIn?: (event: MouseEvent<HTMLAnchorElement>) => void
  /** Replaces Jump In, for event actions. */
  actions?: ReactNode
}

const DEFAULT_DESCRIPTION =
  "You're about to jump into Decentraland! Explore a virtual world built by its community and make new friends from anywhere."

function Badge({ badge }: { badge: JumpCardBadge }) {
  if (badge.kind === 'live') {
    return (
      <span className={`${styles.badge} ${styles.live}`}>
        <LiveEventIcon />
        <span>{`LIVE +${badge.count}`}</span>
      </span>
    )
  }
  if (badge.kind === 'attendees') {
    return (
      <span className={styles.badge}>
        <BellIcon className={styles.accent} />
        <span aria-hidden="true">+{badge.count}</span>
        <span className={styles.visuallyHidden}>{badge.count} interested</span>
      </span>
    )
  }
  return (
    <span className={styles.badge}>
      <CircleIcon className={styles.online} />
      <PersonIcon />
      <span aria-hidden="true">{badge.count}</span>
      <span className={styles.visuallyHidden}>{badge.count} players in-world</span>
    </span>
  )
}

/** A fixed-height, scrolling text with a fade over its last line while more follows (sites: TextWrapper). */
function Description({ text }: { text: string }) {
  const ref = useRef<HTMLDivElement>(null)
  const [fade, setFade] = useState(false)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const check = () =>
      setFade(
        el.scrollHeight > el.clientHeight && el.scrollTop + el.clientHeight < el.scrollHeight - 1
      )
    check()
    el.addEventListener('scroll', check)
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(check) : undefined
    observer?.observe(el)
    return () => {
      observer?.disconnect()
      el.removeEventListener('scroll', check)
    }
  }, [text])
  return (
    <div className={styles.text} data-fade={fade}>
      <div ref={ref} className={styles.scroll}>
        <p className={styles.description}>{text}</p>
      </div>
    </div>
  )
}

function JumpCard({
  kind,
  title,
  creator,
  image,
  description,
  location,
  when,
  badge,
  jumpInHref,
  onJumpIn,
  actions
}: JumpCardProps) {
  const titleId = useId()
  const imageSrc = image || (kind === 'event' ? eventPlaceholder : placePlaceholder)
  const imageKind = kind === 'event' ? 'event' : 'place'
  const bottom =
    actions ??
    (jumpInHref ? (
      <Button href={jumpInHref} onClick={onJumpIn} className={styles.jumpIn}>
        Jump In
        <JumpInIcon />
      </Button>
    ) : null)
  const whenClass = [styles.chip, styles.date, when?.ended && styles.ended]
    .filter(Boolean)
    .join(' ')

  return (
    <article className={styles.root} aria-labelledby={titleId}>
      <div className={styles.media}>
        <img
          className={styles.image}
          src={imageSrc}
          alt={
            title
              ? `${title} ${imageKind} image`
              : `${imageKind === 'event' ? 'Event' : 'Place'} image`
          }
        />
        {badge ? <Badge badge={badge} /> : null}
      </div>
      <div className={styles.body}>
        <div className={styles.content}>
          <h1 className={styles.title} id={titleId}>
            {title}
          </h1>
          <p className={styles.creator}>
            <img
              className={styles.avatar}
              src={creator.avatar || (creator.foundation ? decentralandLogo : creatorPlaceholder)}
              alt=""
              width={32}
              height={32}
              style={
                creator.color ? ({ '--avatar-bg': creator.color } as CSSProperties) : undefined
              }
            />
            <span className={styles.by}>By </span>
            {creator.href ? (
              <a
                className={styles.creatorLink}
                href={creator.href}
                aria-label={`View ${creator.name}'s profile`}
              >
                {creator.name}
              </a>
            ) : (
              <span>{creator.name}</span>
            )}
          </p>
          <div className={styles.meta}>
            {when ? (
              <span className={whenClass}>
                <AccessTimeIcon />
                {when.dateTime ? <time dateTime={when.dateTime}>{when.label}</time> : when.label}
              </span>
            ) : null}
            <span className={styles.chip}>
              {location.world ? (
                <>
                  <PublicIcon />
                  <span className={styles.visuallyHidden}>World: </span>
                  {location.world}
                </>
              ) : (
                <>
                  <PlaceIcon />
                  <span className={styles.visuallyHidden}>Parcel: </span>
                  {location.parcel}
                </>
              )}
            </span>
          </div>
          <Description text={description || DEFAULT_DESCRIPTION} />
        </div>
        {bottom ? <div className={styles.bottom}>{bottom}</div> : null}
      </div>
    </article>
  )
}

export { JumpCard }
export type { JumpCardBadge, JumpCardProps }
