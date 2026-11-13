// TODO(figma-ds): replace with the Overview tab's cards, until the Figma-generated profile overview from library/ui/components.
import type { ReactNode } from 'react'
import { Icon } from './icons'
import styles from './ProfileOverview.module.css'

type ProfileInfoItem = { key: string; label: string; value: string; icon?: ReactNode }
type ProfileLinkItem = { title: string; url: string }
type ProfileBadgeItem = { id: string; name: string; image: string | null }

type ProfileOverviewProps = {
  bio: string
  /** Shown in place of the bio when there is none; it differs for the owner and for a visitor. */
  emptyBio: string
  info: readonly ProfileInfoItem[]
  links: readonly ProfileLinkItem[]
  badges?: readonly ProfileBadgeItem[]
  /** The worn items that are not base-avatar defaults. */
  equipped?: readonly string[]
  /** The owner's Edit button opens the avatar editor. */
  onEdit?: () => void
  /** Further cards below the two the profile site shows. */
  children?: ReactNode
}

/** Only http(s) links reach the page: a profile's links are written by its owner. */
function safeUrl(url: string): string | null {
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.href : null
  } catch {
    return null
  }
}

/** The site name a link points to, for a link saved without a title. */
function linkProvider(url: string): string {
  const host = new URL(url).hostname.replace(/^www\./, '')
  const first = host.split('.')[0] ?? host
  return first.charAt(0).toUpperCase() + first.slice(1)
}

/** The Overview tab's two translucent cards: Badges, About and Links, then Equipped items. */
function ProfileOverview({
  bio,
  emptyBio,
  info,
  links,
  badges = [],
  equipped = [],
  onEdit,
  children
}: ProfileOverviewProps) {
  const safeLinks = links.flatMap((link) => {
    const url = safeUrl(link.url)
    return url
      ? [{ title: link.title && link.title !== link.url ? link.title : linkProvider(url), url }]
      : []
  })
  const text = bio.trim()
  const edit = onEdit ? (
    <button type="button" className={styles.editButton} onClick={onEdit}>
      Edit
      <span className={styles.editIcon}>
        <Icon name="EditOutlined" />
      </span>
    </button>
  ) : null

  return (
    <div className={styles.overview}>
      <div className={styles.surface}>
        {edit}
        <section className={styles.section}>
          <div className={styles.sectionHeader}>
            <h2 className={styles.sectionTitle}>Badges</h2>
          </div>
          {badges.length > 0 ? (
            <div className={styles.badges}>
              {badges.map((badge) => (
                <div key={badge.id} className={styles.badgeSlot} title={badge.name}>
                  {badge.image ? (
                    <img
                      className={styles.badgeImage}
                      src={badge.image}
                      alt={badge.name}
                      loading="lazy"
                    />
                  ) : (
                    <span className={styles.badgeFallback} role="img" aria-label={badge.name}>
                      {badge.name.charAt(0).toUpperCase()}
                    </span>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <p className={styles.empty}>
              No badges yet. Jump in world and start collecting badges!
            </p>
          )}
        </section>
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>About</h2>
          {text ? (
            <p className={`${styles.bio} ${styles.spaced}`}>{text}</p>
          ) : (
            <p className={`${styles.empty} ${styles.spaced}`}>{emptyBio}</p>
          )}
          {info.length > 0 ? (
            <div className={styles.infoGrid}>
              {info.map((item) => (
                <div key={item.key} className={styles.infoItem}>
                  <p className={styles.infoLabel}>
                    {item.icon ? <span className={styles.infoIcon}>{item.icon}</span> : null}
                    {item.label}
                  </p>
                  <p className={styles.infoValue}>{item.value}</p>
                </div>
              ))}
            </div>
          ) : null}
        </section>
        {safeLinks.length > 0 ? (
          <section className={styles.section}>
            <h2 className={styles.sectionTitle}>Links</h2>
            <div className={`${styles.links} ${styles.spaced}`}>
              {safeLinks.map((link) => (
                <a
                  key={link.url}
                  className={styles.linkPill}
                  href={link.url}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <span className={styles.linkPillIcon}>
                    <Icon name="InsertLink" />
                  </span>
                  {link.title}
                </a>
              ))}
            </div>
          </section>
        ) : null}
      </div>
      <div className={styles.surface}>
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>Equipped items</h2>
          {equipped.length > 0 ? (
            <ul className={`${styles.equipped} ${styles.spaced}`}>
              {equipped.map((urn) => (
                <li key={urn} className={styles.equippedItem}>
                  {urn}
                </li>
              ))}
            </ul>
          ) : (
            <p className={`${styles.empty} ${styles.spaced}`}>No equipped items yet.</p>
          )}
        </section>
      </div>
      {children}
    </div>
  )
}

export { ProfileOverview, safeUrl }
export type { ProfileBadgeItem, ProfileInfoItem, ProfileLinkItem, ProfileOverviewProps }
