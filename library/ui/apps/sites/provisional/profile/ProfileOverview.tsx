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
  /** Further cards below About (badges, equipped items). */
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

/** About, the profile's details and its links, in the source's translucent card. */
function ProfileOverview({ bio, emptyBio, info, links, children }: ProfileOverviewProps) {
  const safeLinks = links.flatMap((link) => {
    const url = safeUrl(link.url)
    return url ? [{ title: link.title && link.title !== link.url ? link.title : linkProvider(url), url }] : []
  })
  const text = bio.trim()

  return (
    <div className={styles.root}>
      <section className={styles.surface}>
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
              {safeLinks.map((link) => (
                <li key={link.url}>
                  <a className={styles.link} href={link.url} target="_blank" rel="noopener noreferrer">
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

export { ProfileOverview, safeUrl }
export type { ProfileInfoItem, ProfileLinkItem, ProfileOverviewProps }
