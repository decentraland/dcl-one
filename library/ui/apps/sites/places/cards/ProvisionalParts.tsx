// TODO(shared-card): provisional pieces of the two /places cards. Delete this file and its module
// when PlaceCardSlot and LiveEventCardSlot render the shared design-system cards.
import { useState, type MouseEvent } from 'react'
import {
  GeneralFeatured,
  GeneralJumpIn,
  GeneralLive,
  GeneralPlayers
} from '../../../../foundations/icons'
import dclLogo from '../../assets/dcl-logo.svg'
import type { PlaceCardData, PlaceCardHandlers, PlaceCardLabels } from './types'
import styles from './ProvisionalParts.module.css'

function Badges({ card, labels }: { card: PlaceCardData; labels: PlaceCardLabels }) {
  const users = card.users ?? 0
  return (
    <div className={styles.badges}>
      <div className={styles.badgeGroup}>
        {card.live ? (
          <span className={styles.live} title={card.liveEventName}>
            <GeneralLive size={16} aria-hidden="true" />
            {labels.live}
          </span>
        ) : null}
        {users > 0 ? (
          <span className={styles.users}>
            <span className={styles.dot} aria-hidden="true" />
            <GeneralPlayers size={14} aria-hidden="true" />
            <span aria-hidden="true">{users}</span>
            <span className={styles.visuallyHidden}>{card.usersLabel ?? String(users)}</span>
          </span>
        ) : null}
      </div>
      {card.featured ? (
        <span className={styles.featured}>
          <GeneralFeatured size={14} aria-hidden="true" />
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
  if (src && failed !== src)
    return (
      <img
        className={styles.avatar}
        src={src}
        alt=""
        loading="lazy"
        onError={() => setFailed(src)}
      />
    )
  return (
    <span className={styles.avatar} aria-hidden="true">
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
  className: string
  onOpen?: PlaceCardHandlers['onOpen']
}) {
  return (
    <a
      className={`${styles.titleLink} ${className}`}
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
  className: string
  onJumpIn?: PlaceCardHandlers['onJumpIn']
}) {
  if (!card.jumpHref) return null
  return (
    <a
      className={`${styles.jumpIn} ${className}`}
      href={card.jumpHref}
      aria-label={`${labels.jumpIn}: ${card.title}`}
      onClick={onJumpIn && ((event: MouseEvent<HTMLAnchorElement>) => onJumpIn(card, event))}
    >
      {labels.jumpIn}
      <GeneralJumpIn size={22} aria-hidden="true" />
    </a>
  )
}

function Cover({ card, priority = false }: { card: PlaceCardData; priority?: boolean }) {
  return card.image ? (
    <img className={styles.cover} src={card.image} alt="" loading={priority ? 'eager' : 'lazy'} />
  ) : null
}

export { Badges, ByLine, Cover, JumpIn, TitleLink }
