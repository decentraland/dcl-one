// TODO(figma-ds): replace with the Figma-generated badges row and tooltip from library/ui/components.
import { useId } from 'react'
import styles from './ProfileBadges.module.css'

type ProfileBadgeItem = {
  id: string
  name: string
  imageUrl?: string
  tierName?: string
  description?: string
  /** Already formatted by the caller, e.g. "Unlocked on Mar 3, 2026". */
  unlocked?: string
}

type ProfileBadgesProps = {
  badges: readonly ProfileBadgeItem[]
  loading: boolean
  /** Shown when the member has no badge yet. */
  emptyText: string
  /**
   * Shown instead when the badges could not be read and none are on screen, so a failure does not
   * read as "no badges". The caller passes it only when there is no data to show.
   */
  errorText?: string
}

function Badge({ badge }: { badge: ProfileBadgeItem }) {
  const tipId = useId()
  return (
    <li className={styles.badge}>
      <span className={styles.slot} tabIndex={0} aria-describedby={tipId}>
        {badge.imageUrl ? (
          <img className={styles.image} src={badge.imageUrl} alt={badge.name} loading="lazy" />
        ) : (
          <span className={styles.fallback} role="img" aria-label={badge.name}>
            {badge.name.charAt(0).toUpperCase()}
          </span>
        )}
      </span>
      <span className={styles.tip} role="tooltip" id={tipId}>
        <strong className={styles.tipName}>{badge.name}</strong>
        {badge.tierName ? <span className={styles.tipTier}>{badge.tierName}</span> : null}
        {badge.description ? <span>{badge.description}</span> : null}
        {badge.unlocked ? <span className={styles.tipDate}>{badge.unlocked}</span> : null}
      </span>
    </li>
  )
}

/** The member's badges, each with its name, tier, blurb and unlock date on hover or focus. */
function ProfileBadges({ badges, loading, emptyText, errorText }: ProfileBadgesProps) {
  return (
    <div className={styles.root}>
      <h2 className={styles.title}>Badges</h2>
      {/* One live region, always mounted, so a screen reader hears how the loading ended. */}
      <p
        className={loading ? styles.status : styles.empty}
        role="status"
        hidden={!loading && !errorText && badges.length > 0}
      >
        {loading ? 'Loading badges…' : errorText ? errorText : badges.length > 0 ? '' : emptyText}
      </p>
      {!loading && !errorText && badges.length > 0 ? (
        <ul className={styles.row}>
          {badges.map((badge) => (
            <Badge key={badge.id} badge={badge} />
          ))}
        </ul>
      ) : null}
    </div>
  )
}

export { ProfileBadges }
export type { ProfileBadgeItem, ProfileBadgesProps }
