import { Badges, CardActions, CreatorRow, TimePill } from './parts'
import type { ExperienceCardProps } from './types'
import styles from './cards.module.css'

/**
 * All Events' card in a day column: a live event shows its badges, a later one its start time.
 * TODO(shared-card): render the shared EventCard here; the props stay as they are.
 */
function ExperienceCardSlot({
  title,
  image,
  href,
  onOpen,
  users,
  usersLabel,
  creator,
  live,
  time,
  remind,
  calendarHref
}: ExperienceCardProps) {
  return (
    <article className={`${styles.card} ${styles.dayCard} ${live ? styles.dayCardLive : ''}`}>
      {image || live ? (
        <div className={styles.dayMedia}>
          {image ? (
            <img className={styles.image} src={image} alt="" loading="lazy" decoding="async" />
          ) : null}
          <Badges live={live} users={live ? users : undefined} usersLabel={usersLabel} />
        </div>
      ) : null}
      <div className={styles.dayBody}>
        <h3 className={styles.dayTitle}>
          <a className={styles.cover} href={href} onClick={onOpen}>
            {title}
          </a>
        </h3>
        {creator ? <CreatorRow creator={creator} prefix="By" /> : null}
        {live ? null : <TimePill time={time} />}
      </div>
      {live ? null : <CardActions title={title} remind={remind} calendarHref={calendarHref} />}
    </article>
  )
}

export { ExperienceCardSlot }
