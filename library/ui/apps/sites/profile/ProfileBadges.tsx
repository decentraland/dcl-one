import { useState } from 'react'

import { Tooltip } from '../../../components/Tooltip/Tooltip'
import styles from './ProfileBadges.module.css'

export type ProfileBadgeItem = {
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

/** The widest the tip gets before its text wraps. */
const TIP_MAX_WIDTH = 240

function Badge({ badge }: { badge: ProfileBadgeItem }) {
  // The slot is the tip's anchor, so the open tip describes the focused slot itself.
  const [slot, setSlot] = useState<HTMLSpanElement | null>(null)
  return (
    <li>
      <Tooltip
        anchor={slot}
        side="top"
        gap={8}
        keepInViewport
        maxWidth={TIP_MAX_WIDTH}
        tipClassName={styles.tip}
        content={
          <span className={styles.tipContent}>
            <strong className={styles.tipName}>{badge.name}</strong>
            {badge.tierName ? <span className={styles.tipTier}>{badge.tierName}</span> : null}
            {badge.description ? <span>{badge.description}</span> : null}
            {badge.unlocked ? <span className={styles.tipDate}>{badge.unlocked}</span> : null}
          </span>
        }
      >
        <span ref={setSlot} className={styles.slot} tabIndex={0}>
          {badge.imageUrl ? (
            <img className={styles.image} src={badge.imageUrl} alt={badge.name} loading="lazy" />
          ) : (
            <span className={styles.fallback} role="img" aria-label={badge.name}>
              {badge.name.charAt(0).toUpperCase()}
            </span>
          )}
        </span>
      </Tooltip>
    </li>
  )
}

/** The member's badges, each with its name, tier, blurb and unlock date on hover or focus. */
export function ProfileBadges({ badges, loading, emptyText, errorText }: ProfileBadgesProps) {
  const listed = !loading && !errorText && badges.length > 0
  return (
    <div className={styles.root}>
      <h2 className={styles.title}>Badges</h2>
      {/* One live region, always mounted, so a screen reader hears how the loading ended. */}
      <p className={listed ? styles.loaded : loading ? styles.status : styles.empty} role="status">
        {loading
          ? 'Loading badges…'
          : errorText
            ? errorText
            : badges.length > 0
              ? `${badges.length} ${badges.length === 1 ? 'badge' : 'badges'}`
              : emptyText}
      </p>
      {listed ? (
        <ul className={styles.row}>
          {badges.map((badge) => (
            <Badge key={badge.id} badge={badge} />
          ))}
        </ul>
      ) : null}
    </div>
  )
}
