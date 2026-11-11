import { useId } from 'react'
import { GeneralLive } from '../../../foundations/icons'
import { LiveEventCardSlot } from './cards/LiveEventCardSlot'
import type { LiveEventCardProps } from './cards/types'
import { Rail } from './Rail'
import styles from './LiveNow.module.css'

/** Live events and busy places. Renders nothing when there are none, as source. */
function LiveNow({ cards }: { cards: readonly LiveEventCardProps[] }) {
  const titleId = useId()
  if (!cards.length) return null
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.title}>
        <GeneralLive size={28} aria-hidden="true" className={styles.icon} />
        Live Now
      </h2>
      <Rail label="live scenes" count={cards.length} trackClassName={styles.track}>
        {cards.map((card) => (
          <div key={card.id} className={styles.slide}>
            <LiveEventCardSlot {...card} />
          </div>
        ))}
      </Rail>
    </section>
  )
}

export { LiveNow }
