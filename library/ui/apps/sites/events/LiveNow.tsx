import { useId } from 'react'
import { GeneralLive } from '../../../foundations/icons'
import { LiveNowCard } from './cards'
import type { LiveEventCardProps } from './types'
import { Rail } from './Rail'
import { useEventsCopy } from './locale'
import styles from './LiveNow.module.css'

/** Live events and busy places. Renders nothing when there are none, as source. */
function LiveNow({ cards }: { cards: readonly LiveEventCardProps[] }) {
  const { text } = useEventsCopy()
  const titleId = useId()
  if (!cards.length) return null
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.title}>
        <GeneralLive size={28} aria-hidden="true" className={styles.icon} />
        {text('Live Now')}
      </h2>
      <Rail
        label={text('live scenes')}
        previousLabel={text('Show previous live scenes')}
        nextLabel={text('Show next live scenes')}
        count={cards.length}
        trackClassName={styles.track}
      >
        {cards.map((card, index) => (
          <div key={card.id} className={styles.slide}>
            <LiveNowCard {...card} priority={index === 0} />
          </div>
        ))}
      </Rail>
    </section>
  )
}

export { LiveNow }
