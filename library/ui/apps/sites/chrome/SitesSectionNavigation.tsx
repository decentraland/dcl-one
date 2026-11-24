import type { ReactNode } from 'react'
import ChromeLink from '../../../components/AppChrome/ChromeLink'
import styles from './SitesSectionNavigation.module.css'

export type SitesSectionItem = { id: string; label: ReactNode; href?: string }

export default function SitesSectionNavigation({
  title,
  items,
  active,
  onSelect,
  closeMenu
}: {
  title: string
  items: readonly SitesSectionItem[]
  active?: string
  onSelect?: (id: string) => void
  closeMenu: () => void
}) {
  return (
    <nav aria-label={title + ' sections'} className={styles.root}>
      <div className={styles.title}>{title}</div>
      {items.map((item) => {
        const current = item.id === active ? ('page' as const) : undefined
        return item.href ? (
          <ChromeLink
            key={item.id}
            href={item.href}
            className={styles.item}
            aria-current={current}
            onClick={closeMenu}
          >
            {item.label}
          </ChromeLink>
        ) : (
          <button
            key={item.id}
            type="button"
            className={styles.item}
            aria-current={current}
            onClick={() => {
              onSelect?.(item.id)
              closeMenu()
            }}
          >
            {item.label}
          </button>
        )
      })}
    </nav>
  )
}
