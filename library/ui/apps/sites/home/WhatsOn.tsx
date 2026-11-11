import { useId } from 'react'
import { Button } from '../shared/Button/Button'
import { GeneralLive, GeneralPlayers, GeneralRightArrow } from '../../../foundations/icons'
import dclLogo from '../assets/dcl-logo.svg'
import styles from './WhatsOn.module.css'

type WhatsOnCardData = {
  id: string
  title: string
  image: string | null
  /** The event or place page; the whole card follows it. */
  href: string
  users?: number
  /** Names the user count for assistive technology, e.g. "12 online". */
  usersLabel?: string
  /** `foundation` shows the Decentraland logo as the avatar, as source does for Genesis Plaza. */
  creator?: { name: string; href?: string; avatarUrl?: string; foundation?: boolean }
}

type WhatsOnProps = {
  title: string
  cards: readonly WhatsOnCardData[]
  labels: { live: string; by: string }
  viewAll: { label: string; href: string; onClick?: () => void }
  /** Called when a card's page link is followed, e.g. for analytics. */
  onCardClick?: (card: WhatsOnCardData) => void
}

function CreatorAvatar({ creator }: { creator: NonNullable<WhatsOnCardData['creator']> }) {
  const src = creator.avatarUrl ?? (creator.foundation ? dclLogo : undefined)
  if (src) return <img className={styles.avatar} src={src} alt="" />
  // Source's fallback face: the name's initial on a colored disc.
  return (
    <span className={styles.avatar} aria-hidden="true">
      {creator.name.charAt(0).toUpperCase()}
    </span>
  )
}

/** Source's event card: cover with LIVE and user-count badges, title, then "by <creator>". The title
    link covers the card, so the whole card opens the page; the creator link sits above it. */
function Card({
  card,
  labels,
  onClick
}: {
  card: WhatsOnCardData
  labels: WhatsOnProps['labels']
  onClick?: () => void
}) {
  return (
    <li className={styles.card}>
      <div className={styles.media}>
        {card.image ? (
          <img className={styles.image} src={card.image} alt="" loading="lazy" />
        ) : null}
        <div className={styles.badges}>
          <span className={styles.live}>
            <GeneralLive size={18} aria-hidden="true" />
            {labels.live}
          </span>
          {card.users !== undefined ? (
            <span className={styles.users}>
              <span className={styles.dot} aria-hidden="true" />
              <GeneralPlayers size={14} aria-hidden="true" />
              <span aria-hidden="true">{card.users}</span>
              <span className={styles.visuallyHidden}>{card.usersLabel}</span>
            </span>
          ) : null}
        </div>
      </div>
      <div className={styles.body}>
        <h3 className={styles.cardTitle}>
          <a className={styles.titleLink} href={card.href} onClick={onClick}>
            {card.title}
          </a>
        </h3>
        {card.creator ? (
          <div className={styles.creatorRow}>
            <CreatorAvatar creator={card.creator} />
            <p className={styles.by}>
              {labels.by}{' '}
              {card.creator.href ? (
                <a className={styles.creator} href={card.creator.href}>
                  {card.creator.name}
                </a>
              ) : (
                <span className={styles.creator}>{card.creator.name}</span>
              )}
            </p>
          </div>
        ) : null}
      </div>
    </li>
  )
}

/** "What's on": up to three live places and events. */
function WhatsOn({ title, cards, labels, viewAll, onCardClick }: WhatsOnProps) {
  const titleId = useId()
  if (!cards.length) return null
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.title}>
        {title}
      </h2>
      <ul className={styles.cards}>
        {cards.map((card) => (
          <Card
            key={card.id}
            card={card}
            labels={labels}
            onClick={onCardClick && (() => onCardClick(card))}
          />
        ))}
      </ul>
      <Button variant="tertiary" href={viewAll.href} onClick={viewAll.onClick}>
        {viewAll.label}
        <GeneralRightArrow size={24} aria-hidden="true" />
      </Button>
    </section>
  )
}

export { WhatsOn }
export type { WhatsOnCardData, WhatsOnProps }
