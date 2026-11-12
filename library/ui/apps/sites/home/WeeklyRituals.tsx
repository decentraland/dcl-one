import { useId } from 'react'
import { Carousel, type CarouselLabels } from '../shared/Carousel'
import styles from './WeeklyRituals.module.css'

type RitualCard = {
  id: string
  /** The card's text is baked into its image, so the title is the image's alternative text. */
  title: string
  href: string
  image: string
  mobileImage: string
}

type WeeklyRitualsProps = {
  title: string
  cards: readonly RitualCard[]
  labels: CarouselLabels
  /** Called when a ritual's link is followed, e.g. for analytics. */
  onCardClick?: (card: RitualCard) => void
}

function RitualImage({ card, decorative }: { card: RitualCard; decorative?: boolean }) {
  return (
    <picture>
      {/* Source's sm breakpoint (599px) swaps in the portrait artwork. */}
      <source media="(max-width: 599px)" srcSet={card.mobileImage} />
      <img
        className={styles.image}
        src={card.image}
        alt={decorative ? '' : card.title}
        loading="lazy"
        draggable={false}
      />
    </picture>
  )
}

/**
 * "Your Weekly Rituals": the recurring events as source's carousel, with only the current card
 * interactive.
 */
function WeeklyRituals({ title, cards, labels, onCardClick }: WeeklyRitualsProps) {
  const titleId = useId()
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <div className={styles.inner}>
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        <Carousel
          items={cards}
          autoplayDelay={4000}
          getKey={(card) => card.id}
          labelledBy={titleId}
          labels={labels}
          slideClassName={styles.slide}
          renderSlide={(card, peek) => (
            // Every slide is a link, as source's card; the Carousel keeps the peeks inert.
            <a
              className={styles.link}
              href={card.href}
              tabIndex={peek ? -1 : undefined}
              onClick={() => onCardClick?.(card)}
              draggable={false}
            >
              <RitualImage card={card} decorative={peek} />
            </a>
          )}
        />
      </div>
    </section>
  )
}

export { WeeklyRituals }
export type { RitualCard, WeeklyRitualsProps }
