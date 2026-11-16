// TODO(figma-ds): replace with the Figma-generated profile overview from library/ui/components.
import type { ReactNode } from 'react'
import { GeneralLink } from '../../../../foundations/icons'
import styles from './ProfileOverview.module.css'

type ProfileInfoItem = { key: string; label: string; value: string; icon?: ReactNode }
type ProfileLinkItem = { title: string; url: string }

type ProfileOverviewProps = {
  bio: string
  /** Shown in place of the bio when there is none; it differs for the owner and for a visitor. */
  emptyBio: string
  info: readonly ProfileInfoItem[]
  links: readonly ProfileLinkItem[]
  /** Shown first in the card, as the source orders it: the badges row. */
  badges?: ReactNode
  /** Further cards below the About card (equipped items, for example). */
  children?: ReactNode
}

/** A second safety net for any caller: only http(s) links are rendered. */
function isHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
  } catch {
    return false
  }
}

/** About, the profile's details and its links, in the source's translucent card. */
function ProfileOverview({ bio, emptyBio, info, links, badges, children }: ProfileOverviewProps) {
  // The caller decides each link's shown title; this only refuses a non-http(s) one.
  const safeLinks = links.filter((link) => isHttpUrl(link.url))
  const text = bio.trim()

  return (
    <div className={styles.root}>
      <section className={styles.surface}>
        {badges ? <div className={styles.section}>{badges}</div> : null}
        <div className={styles.section}>
          <h2 className={styles.title}>About</h2>
          {text ? <p className={styles.bio}>{text}</p> : <p className={styles.empty}>{emptyBio}</p>}
          {info.length > 0 ? (
            <dl className={styles.info}>
              {info.map((item) => (
                <div key={item.key} className={styles.infoItem}>
                  <dt className={styles.infoLabel}>
                    {item.icon ? <span className={styles.infoIcon}>{item.icon}</span> : null}
                    {item.label}
                  </dt>
                  <dd className={styles.infoValue}>{item.value}</dd>
                </div>
              ))}
            </dl>
          ) : null}
        </div>
        {safeLinks.length > 0 ? (
          <div className={styles.section}>
            <h2 className={styles.title}>Links</h2>
            <ul className={styles.links}>
              {safeLinks.map((link, index) => (
                <li key={`${index}-${link.url}`}>
                  <a
                    className={styles.link}
                    href={link.url}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <GeneralLink size={22} aria-hidden="true" />
                    {link.title}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </section>
      {children}
    </div>
  )
}

export { ProfileOverview }
export type { ProfileInfoItem, ProfileLinkItem, ProfileOverviewProps }
