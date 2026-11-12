// TODO(figma-ds): replace with the tab bar, until the Figma-generated one from library/ui/components.
import type { ComponentType, ReactNode } from 'react'
import styles from './ProfileTabs.module.css'

type ProfileTabLink = { id: string; label: string; href: string }

/** The app's router link, so a tab change is a client navigation; a plain anchor otherwise. */
type TabLinkProps = {
  to: string
  className: string
  'aria-current'?: 'page'
  onClick?: () => void
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

/** The desktop tab bar: each tab a link to its own path, the current one underlined in the primary colour. */
function ProfileTabs({
  tabs,
  active,
  onSelect,
  LinkComponent = AnchorLink,
  label = 'Profile sections'
}: ProfileTabsProps) {
  return (
    <nav className={styles.tabs} aria-label={label}>
      {tabs.map((tab) => (
        <LinkComponent
          key={tab.id}
          to={tab.href}
          className={styles.tab ?? ''}
          aria-current={tab.id === active ? 'page' : undefined}
          onClick={() => onSelect?.(tab.id)}
        >
          {tab.label}
        </LinkComponent>
      ))}
    </nav>
  )
}

export { ProfileTabs }
export type { ProfileTabLink, ProfileTabsProps, TabLinkProps }
