import { GeneralJumpIn } from '../../../../foundations/icons'
import { Badges, CreatorRow } from './parts'
import type { LiveEventCardProps } from './types'
import styles from './cards.module.css'

/**
 * Live Now's card: cover with LIVE and player badges, title, creator, and Jump In.
 * TODO(shared-card): render the shared EventCard (live events) and PlaceCard (places) from the
 * "UI Components migration" here; the props stay as they are.
 */
function LiveEventCardSlot({
  title,
  image,
  href,
  onOpen,
  users,
  usersLabel,
  creator,
  live,
  jumpIn,
  eager
}: LiveEventCardProps) {
  return (
    <article className={`${styles.card} ${styles.liveCard}`}>
      <div className={styles.liveMedia}>
        {image ? (
          <img
            className={styles.image}
            src={image}
            alt=""
            loading={eager ? 'eager' : 'lazy'}
            fetchPriority={eager ? 'high' : undefined}
            decoding="async"
          />
        ) : null}
        <Badges live={live} users={users} usersLabel={usersLabel} />
      </div>
      <div className={styles.liveBody}>
        <h3 className={styles.liveTitle}>
          <a className={styles.cover} href={href} onClick={onOpen}>
            {title}
          </a>
        </h3>
        {creator ? <CreatorRow creator={creator} /> : null}
        {jumpIn ? (
          <a className={styles.jumpIn} href={jumpIn.href} onClick={jumpIn.onJumpIn}>
            Jump In
            <GeneralJumpIn size={20} aria-hidden="true" />
          </a>
        ) : null}
      </div>
    </article>
  )
}

export { LiveEventCardSlot }
