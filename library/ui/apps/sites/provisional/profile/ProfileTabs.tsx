// TODO(figma-ds): replace with the Figma-generated tab bar from library/ui/components.
import type { ComponentType, MouseEvent, ReactNode } from 'react'
import styles from './ProfileTabs.module.css'

type ProfileTabLink = { id: string; label: string; href: string }

/** The app's router link, so a tab change is a client navigation; a plain anchor otherwise. */
type TabLinkProps = {
  to: string
  className: string
  'aria-current'?: 'page'
  onClick?: (event: MouseEvent<HTMLAnchorElement>) => void
  children: ReactNode
}

type ProfileTabsProps = {
  tabs: readonly ProfileTabLink[]
  active: string
  /** Called with the tab's id when it is chosen. */
  onSelect?: (id: string) => void
  LinkComponent?: ComponentType<TabLinkProps>
  label?: string
}

function AnchorLink({ to, ...props }: TabLinkProps) {
  return <a href={to} {...props} />
}

/** Each tab is a link to its own path; the active one is underlined in the primary color. */
function ProfileTabs({
  tabs,
  active,
  onSelect,
  LinkComponent = AnchorLink,
  label = 'Profile sections'
}: ProfileTabsProps) {
  return (
    <nav className={styles.root} aria-label={label}>
      <ul className={styles.list}>
        {tabs.map((tab) => {
          const current = tab.id === active
          return (
            <li key={tab.id} className={styles.item}>
              <LinkComponent
                to={tab.href}
                className={current ? `${styles.tab} ${styles.active}` : styles.tab}
                aria-current={current ? 'page' : undefined}
                onClick={(event) => {
                  // A modifier or middle click opens a new browser tab; this page's tab does not change.
                  if (
                    event.button !== 0 ||
                    event.metaKey ||
                    event.ctrlKey ||
                    event.shiftKey ||
                    event.altKey
                  )
                    return
                  onSelect?.(tab.id)
                }}
              >
                {tab.label}
              </LinkComponent>
            </li>
          )
        })}
      </ul>
    </nav>
  )
}

export { ProfileTabs }
export type { ProfileTabLink, ProfileTabsProps, TabLinkProps }
