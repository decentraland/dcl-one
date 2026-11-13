// TODO(shared-card): provisional pieces of the two /places cards. Delete this file and its module
// when PlaceCardSlot and LiveEventCardSlot render the shared design-system cards.
import { useState, type CSSProperties, type MouseEvent } from 'react'
import dclLogo from '../../assets/dcl-logo.svg'
import type { PlaceCardData, PlaceCardHandlers, PlaceCardLabels } from './types'
import styles from './ProvisionalParts.module.css'

/* The reference's badge and button glyphs, inline so they measure and pair as it does. */
function SensorsGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
      data-testid="SensorsRoundedIcon"
    >
      <path d="M8.54 8.54c.35.35.37.88.1 1.29C8.24 10.45 8 11.2 8 12c0 .8.24 1.55.64 2.17.27.41.24.95-.11 1.29-.43.43-1.17.4-1.51-.11C6.38 14.4 6 13.24 6 12c0-1.21.36-2.33.97-3.28.36-.54 1.11-.64 1.57-.18m6.92 6.92c.43.43 1.17.4 1.51-.11C17.62 14.4 18 13.24 18 12c0-1.24-.38-2.4-1.03-3.36-.34-.5-1.08-.54-1.51-.11-.35.35-.37.88-.11 1.29.41.63.65 1.38.65 2.18 0 .8-.24 1.55-.64 2.17-.27.41-.24.95.1 1.29M12 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2m6.32 8.32c.42.42 1.12.39 1.5-.08C21.18 16.53 22 14.36 22 12s-.82-4.53-2.18-6.24c-.37-.47-1.07-.5-1.5-.08-.36.36-.4.92-.08 1.32 1.1 1.37 1.76 3.11 1.76 5s-.66 3.63-1.76 5c-.32.39-.28.96.08 1.32M5.68 5.68c-.42-.42-1.12-.39-1.5.08C2.82 7.47 2 9.64 2 12s.82 4.53 2.18 6.24c.37.47 1.07.5 1.5.08.36-.36.4-.92.08-1.32C4.66 15.63 4 13.89 4 12s.66-3.63 1.76-5c.32-.39.28-.96-.08-1.32" />
    </svg>
  )
}

function PlayersGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
      data-testid="PermIdentityRoundedIcon"
    >
      <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4m0-6c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2 .9-2 2-2m0 7c-2.67 0-8 1.34-8 4v2c0 .55.45 1 1 1h14c.55 0 1-.45 1-1v-2c0-2.66-5.33-4-8-4m6 5H6v-.99c.2-.72 3.3-2.01 6-2.01s5.8 1.29 6 2z" />
    </svg>
  )
}

function MedalGlyph() {
  return (
    <svg viewBox="0 0 12 14" fill="none" aria-hidden="true" focusable="false">
      <path
        d="M1.70573 8.60973C2.65715 9.71845 4.02892 10.4545 5.57266 10.5702C5.57069 10.5747 5.56812 10.5793 5.56585 10.5838L3.91221 13.8646C3.82445 14.0387 3.57919 14.0467 3.48104 13.8782L2.48145 12.1608C2.43368 12.0787 2.34327 12.0317 2.24883 12.0398L0.26719 12.2096C0.07458 12.2261 -0.0608893 12.0235 0.0277572 11.8514L1.70573 8.60973Z"
        fill="currentColor"
        stroke="currentColor"
      />
      <path
        d="M11.9721 11.8514C12.0608 12.0234 11.9261 12.2257 11.7337 12.2096L9.75203 12.0398C9.6575 12.0316 9.56624 12.0786 9.51843 12.1608L8.51885 13.8782C8.42065 14.0465 8.17542 14.0387 8.08768 13.8646L6.43404 10.5838C6.4318 10.5794 6.43014 10.5747 6.4282 10.5702C7.972 10.4545 9.34374 9.71857 10.2951 8.60973L11.9721 11.8514Z"
        fill="currentColor"
        stroke="currentColor"
      />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M5.99994 0C8.70388 0 10.8964 2.19753 10.8966 4.90842C10.8966 7.61953 8.70404 9.81781 5.99994 9.81781C3.29598 9.81766 1.10423 7.61943 1.10423 4.90842C1.10449 2.19762 3.29614 0.000150887 5.99994 0ZM6.21699 2.22489C6.12476 2.05002 5.875 2.04987 5.7829 2.22489L5.11229 3.49932C5.07673 3.5667 5.01203 3.61416 4.9371 3.62715L3.52094 3.87209C3.32669 3.90604 3.24927 4.14492 3.38663 4.28681L4.38815 5.31924C4.44118 5.37394 4.46608 5.45061 4.45531 5.52612L4.25092 6.95278C4.22321 7.14842 4.42528 7.29554 4.60228 7.20844L5.89191 6.5722C5.9602 6.53861 6.04069 6.53855 6.10895 6.5722L7.39761 7.20844C7.57464 7.29572 7.77761 7.14846 7.74994 6.95278L7.54458 5.52612C7.53384 5.45064 7.55873 5.37392 7.61173 5.31924L8.61326 4.28681C8.75088 4.14485 8.6735 3.90584 8.47895 3.87209L7.06279 3.62715C6.98794 3.61404 6.92307 3.5667 6.8876 3.49932L6.21699 2.22489Z"
        fill="currentColor"
        stroke="currentColor"
      />
    </svg>
  )
}

function JumpInGlyph() {
  return (
    <svg viewBox="0 0 24.8735 24.8735" fill="none" aria-hidden="true" focusable="false">
      <rect
        width="24.8735"
        height="24.8735"
        rx="8.29118"
        style={{ fill: 'var(--dcl-action-primary)' }}
      />
      <rect
        x="0.777298"
        y="0.777298"
        width="23.3189"
        height="23.3189"
        rx="7.51388"
        stroke="currentColor"
        strokeOpacity="0.3"
        strokeWidth="1.5546"
      />
      <path
        d="M18.9554 11.031L14.1081 6.1861C13.244 5.32244 11.7898 5.93332 11.7898 7.15508V8.75601C11.7476 8.75601 11.7265 8.75601 11.6844 8.75601H7.08641C6.30662 8.75601 5.67436 9.36689 5.67436 10.1463V13.8326C5.67436 14.612 6.30662 15.244 7.08641 15.244H11.6633C11.7055 15.244 11.7265 15.244 11.7687 15.244V16.8449C11.7687 18.0667 13.244 18.6776 14.087 17.8139L18.9343 12.969C19.4823 12.4213 19.4823 11.5576 18.9554 11.031Z"
        style={{ fill: 'var(--dcl-neutrals-white)' }}
      />
    </svg>
  )
}

function Badges({ card, labels }: { card: PlaceCardData; labels: PlaceCardLabels }) {
  const users = card.users ?? 0
  return (
    <div className={styles.badges}>
      <div className={styles.badgeGroup}>
        {card.live ? (
          <span className={styles.live} title={card.liveEventName}>
            <SensorsGlyph />
            {labels.live}
          </span>
        ) : null}
        {users > 0 ? (
          <span className={styles.users} role="img" aria-label={card.usersLabel ?? String(users)}>
            <span className={styles.dot} aria-hidden="true" />
            <PlayersGlyph />
            {users}
          </span>
        ) : null}
      </div>
      {card.featured ? (
        <span className={styles.featured}>
          <MedalGlyph />
          {labels.featured}
        </span>
      ) : null}
    </div>
  )
}

function Avatar({ creator }: { creator: NonNullable<PlaceCardData['creator']> }) {
  const src = creator.foundation ? dclLogo : creator.avatarUrl
  // A face the CDN no longer serves falls back to the initial instead of a broken image.
  const [failed, setFailed] = useState<string | null>(null)
  const style = creator.avatarBackground
    ? ({ '--avatar-bg': creator.avatarBackground } as CSSProperties)
    : undefined
  if (src && failed !== src)
    return (
      <img
        className={styles.avatar}
        style={style}
        src={src}
        alt=""
        loading="lazy"
        onError={() => setFailed(src)}
      />
    )
  return (
    <span className={styles.avatar} style={style} aria-hidden="true">
      {creator.name.charAt(0).toUpperCase()}
    </span>
  )
}

function ByLine({ card, labels }: { card: PlaceCardData; labels: PlaceCardLabels }) {
  const { creator } = card
  if (!creator) return <span />
  return (
    <span className={styles.creatorRow}>
      <Avatar creator={creator} />
      <span className={styles.by}>
        {labels.by}{' '}
        {creator.href ? (
          <a className={styles.creator} href={creator.href}>
            {creator.name}
          </a>
        ) : (
          <span className={styles.creator}>{creator.name}</span>
        )}
      </span>
    </span>
  )
}

/** The title link covers the card, so the whole card opens the page; links above it keep their own target. */
function TitleLink({
  card,
  className,
  onOpen
}: {
  card: PlaceCardData
  className?: string
  onOpen?: PlaceCardHandlers['onOpen']
}) {
  return (
    <a
      className={`${styles.titleLink} ${className ?? ''}`}
      href={card.href}
      onClick={onOpen && ((event: MouseEvent<HTMLAnchorElement>) => onOpen(card, event))}
    >
      {card.title}
    </a>
  )
}

function JumpIn({
  card,
  labels,
  className,
  onJumpIn
}: {
  card: PlaceCardData
  labels: PlaceCardLabels
  className?: string
  onJumpIn?: PlaceCardHandlers['onJumpIn']
}) {
  if (!card.jumpHref) return null
  return (
    <a
      className={`${styles.jumpIn} ${className ?? ''}`}
      href={card.jumpHref}
      aria-label={`${labels.jumpIn}: ${card.title}`}
      onClick={onJumpIn && ((event: MouseEvent<HTMLAnchorElement>) => onJumpIn(card, event))}
    >
      {labels.jumpIn}
      <JumpInGlyph />
    </a>
  )
}

/** The cover is a background image, as the reference's: an <img> resamples and snaps its edges differently. */
function Cover({ card }: { card: PlaceCardData }) {
  if (!card.image) return null
  const url = card.image.replace(/[\\"]/g, '\\$&').replace(/[\r\n]/g, '')
  return <div className={styles.cover} style={{ backgroundImage: `url("${url}")` }} aria-hidden="true" />
}

export { Badges, ByLine, Cover, JumpIn, TitleLink }
