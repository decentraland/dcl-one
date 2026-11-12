import { useId, type ReactNode } from 'react'
import { JumpInCTA } from '../../../components/JumpInCTA/JumpInCTA'
import { Button } from '../shared/Button/Button'
import { Carousel, type CarouselLabels } from '../shared/Carousel'
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
  /** The in-world position the hover call jumps to. */
  jumpHref?: string
  /** `foundation` shows the Decentraland logo as the avatar, as source does for Genesis Plaza;
      `color` is the disc behind the face, source's hash of the display name. */
  creator?: {
    name: string
    href?: string
    avatarUrl?: string
    foundation?: boolean
    color?: string
  }
}

type WhatsOnProps = {
  title: string
  cards: readonly WhatsOnCardData[]
  labels: { live: string; by: string; jumpIn: string }
  /** The phone carousel's controls. */
  carouselLabels: CarouselLabels
  viewAll: { label: string; href: string; onClick?: () => void }
  /** Decorative background supplied by the owning route. */
  background?: ReactNode
  /** Called when a card's page link is followed, e.g. for analytics. */
  onCardClick?: (card: WhatsOnCardData) => void
}

// Source draws the badges and the View All chevron with @mui/icons-material (SensorsRounded,
// PermIdentityRounded, ChevronRight); the same paths keep the pixels.
function Icon({ className, d, name }: { className?: string; d: string; name: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      focusable="false"
      aria-hidden="true"
      data-testid={`${name}Icon`}
    >
      <path d={d} fill="currentColor" />
    </svg>
  )
}
const SENSORS =
  'M8.54 8.54c.35.35.37.88.1 1.29C8.24 10.45 8 11.2 8 12c0 .8.24 1.55.64 2.17.27.41.24.95-.11 1.29-.43.43-1.17.4-1.51-.11C6.38 14.4 6 13.24 6 12c0-1.21.36-2.33.97-3.28.36-.54 1.11-.64 1.57-.18m6.92 6.92c.43.43 1.17.4 1.51-.11C17.62 14.4 18 13.24 18 12c0-1.24-.38-2.4-1.03-3.36-.34-.5-1.08-.54-1.51-.11-.35.35-.37.88-.11 1.29.41.63.65 1.38.65 2.18 0 .8-.24 1.55-.64 2.17-.27.41-.24.95.1 1.29M12 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2m6.32 8.32c.42.42 1.12.39 1.5-.08C21.18 16.53 22 14.36 22 12s-.82-4.53-2.18-6.24c-.37-.47-1.07-.5-1.5-.08-.36.36-.4.92-.08 1.32 1.1 1.37 1.76 3.11 1.76 5s-.66 3.63-1.76 5c-.32.39-.28.96.08 1.32M5.68 5.68c-.42-.42-1.12-.39-1.5.08C2.82 7.47 2 9.64 2 12s.82 4.53 2.18 6.24c.37.47 1.07.5 1.5.08.36-.36.4-.92.08-1.32C4.66 15.63 4 13.89 4 12s.66-3.63 1.76-5c.32-.39.28-.96-.08-1.32'
const PERSON =
  'M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4m0-6c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2 .9-2 2-2m0 7c-2.67 0-8 1.34-8 4v2c0 .55.45 1 1 1h14c.55 0 1-.45 1-1v-2c0-2.66-5.33-4-8-4m6 5H6v-.99c.2-.72 3.3-2.01 6-2.01s5.8 1.29 6 2z'
const CHEVRON = 'M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z'

function CreatorAvatar({ creator }: { creator: NonNullable<WhatsOnCardData['creator']> }) {
  const src = creator.avatarUrl ?? (creator.foundation ? dclLogo : undefined)
  return (
    <span
      className={styles.avatar}
      style={creator.color ? { backgroundColor: creator.color } : undefined}
      aria-hidden="true"
    >
      {/* Source's fallback face: the name's initial on the coloured disc. */}
      {src ? (
        <img className={styles.face} src={src} alt="" />
      ) : (
        creator.name.charAt(0).toUpperCase()
      )}
    </span>
  )
}

/** Source's event card: cover with LIVE and user-count badges, title, "by <creator>" and, on hover,
    the jump-in call. The title link covers the card, so the whole card opens the page; the creator
    link and the call sit above it. */
function Card({
  card,
  labels,
  as: Root = 'li',
  onClick
}: {
  card: WhatsOnCardData
  labels: WhatsOnProps['labels']
  as?: 'li' | 'div'
  onClick?: () => void
}) {
  return (
    <Root className={styles.card} data-card={card.title}>
      <div className={styles.paper}>
        <div className={styles.badges}>
          <span className={styles.live}>
            <Icon className={styles.liveIcon} d={SENSORS} name="SensorsRounded" />
            {labels.live}
          </span>
          {card.users !== undefined ? (
            <span className={styles.users}>
              <span className={styles.dot} aria-hidden="true" />
              <Icon className={styles.usersIcon} d={PERSON} name="PermIdentityRounded" />
              <span aria-hidden="true">{card.users}</span>
              <span className={styles.visuallyHidden}>{card.usersLabel}</span>
            </span>
          ) : null}
        </div>
        <div className={styles.mediaBox}>
          <div
            className={styles.media}
            style={card.image ? { backgroundImage: `url("${card.image}")` } : undefined}
          />
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
                {/* One text node, as source's "by " with its space. */}
                {`${labels.by} `}
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
          {card.jumpHref ? (
            <div className={styles.jumpBox}>
              <JumpInCTA className={styles.jump} href={card.jumpHref} label={labels.jumpIn} />
            </div>
          ) : null}
        </div>
      </div>
    </Root>
  )
}

/** "What's on": up to three live places and events, a row on desktop and source's carousel below
    991px; both render and the viewport width picks one, so SSR markup is stable. */
function WhatsOn({
  title,
  cards,
  labels,
  carouselLabels,
  viewAll,
  background,
  onCardClick
}: WhatsOnProps) {
  const titleId = useId()
  if (!cards.length) return null
  const click = (card: WhatsOnCardData) => onCardClick && (() => onCardClick(card))
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      {background ? (
        <div className={styles.background} aria-hidden="true" inert>
          {background}
        </div>
      ) : null}
      <h2 id={titleId} className={styles.title}>
        {title}
      </h2>
      <ul className={styles.grid}>
        {cards.map((card) => (
          <Card key={card.id} card={card} labels={labels} onClick={click(card)} />
        ))}
      </ul>
      <Carousel
        className={styles.carousel}
        items={cards}
        autoplayDelay={5000}
        getKey={(card) => card.id}
        labelledBy={titleId}
        labels={carouselLabels}
        renderSlide={(card) => <Card card={card} labels={labels} as="div" onClick={click(card)} />}
      />
      <Button
        variant="tertiary"
        className={styles.viewAll}
        href={viewAll.href}
        onClick={viewAll.onClick}
      >
        {viewAll.label}
        <Icon className={styles.viewAllIcon} d={CHEVRON} name="ChevronRight" />
      </Button>
    </section>
  )
}

export { WhatsOn }
export type { WhatsOnCardData, WhatsOnProps }
