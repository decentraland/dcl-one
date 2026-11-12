import { CardFooter, CreatorRow } from './parts'
import type { UpcomingEventCardProps } from './types'
import styles from './cards.module.css'

/**
 * Upcoming's card: a cover beside the title, creator and start time.
 * TODO(shared-card): render the shared EventCard (small variant) here; the props stay as they are.
 */
function UpcomingEventCardSlot({
  title,
  image,
  href,
  onOpen,
  creator,
  time,
  remind,
  calendarHref
}: UpcomingEventCardProps) {
  return (
    <article className={`${styles.card} ${styles.smallCard}`}>
      <div className={styles.smallMedia}>
        {image ? (
          <img className={styles.image} src={image} alt="" loading="lazy" decoding="async" />
        ) : null}
      </div>
      <div className={styles.smallBody}>
        <h3 className={styles.smallTitle}>
          <a className={styles.cover} href={href} onClick={onOpen}>
            {title}
          </a>
        </h3>
        {creator ? <CreatorRow creator={creator} /> : null}
        <CardFooter
          title={title}
          href={href}
          time={time}
          remind={remind}
          calendarHref={calendarHref}
        />
      </div>
    </article>
  )
}

export { UpcomingEventCardSlot }
