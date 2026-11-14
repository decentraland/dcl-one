import { useId } from 'react'
import { UpcomingEventCardSlot } from './cards/UpcomingEventCardSlot'
import type { UpcomingEventCardProps } from './cards/types'
import { PageDots, useScrollPages } from './Rail'
import styles from './Upcoming.module.css'

/**
 * Events starting within the day: a four-column grid from desktop; on phones, pages of two cards
 * that scroll sideways with page dots, as source. Renders nothing when there are none.
 */
function Upcoming({ cards }: { cards: readonly UpcomingEventCardProps[] }) {
  const titleId = useId()
  const { ref, pages, active, sync, goTo } = useScrollPages<HTMLUListElement>(cards.length)
  if (!cards.length) return null
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.title}>
        Upcoming
      </h2>
      <ul ref={ref} className={styles.grid} onScroll={sync}>
        {cards.map((card) => (
          <li key={card.id} className={styles.item}>
            <UpcomingEventCardSlot {...card} />
          </li>
        ))}
      </ul>
      <div className={styles.dots}>
        <PageDots pages={pages} active={active} label="Upcoming" onSelect={goTo} />
      </div>
    </section>
  )
}

export { Upcoming }
