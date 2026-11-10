import type { ReactNode } from 'react'
import { inAppLink, type BlogNavigate } from './blogLinks'
import styles from './BlogLayout.module.css'

// The blog's page frame and sub-navigation, as source's BlogLayout and BlogNavigation
// (components/blog/BlogLayout.tsx, components/blog/BlogNavigation).

export type BlogNavCategory = { id: string; title: string; url: string }

export type BlogNavigationProps = {
  /** The categories shown in the menu, in order. */
  categories: readonly BlogNavCategory[]
  /** The href of the current entry: `/blog` for All articles, else a category's url. */
  activeHref?: string
  /** The search box, at the end of the bar. */
  search?: ReactNode
  onNavigate?: BlogNavigate
}

export function BlogNavigation({
  categories,
  activeHref,
  search,
  onNavigate
}: BlogNavigationProps) {
  const entries = [{ id: 'all', title: 'All articles', url: '/blog' }, ...categories]
  return (
    <div className={styles.navBar}>
      <div className={styles.navWrapper}>
        <nav className={styles.navCategories} aria-label="Blog">
          <ul className={styles.navList}>
            {entries.map((entry) => (
              <li key={entry.id} className={styles.navItem}>
                <a
                  className={styles.navLink}
                  href={entry.url}
                  aria-current={entry.url === activeHref ? 'page' : undefined}
                  {...inAppLink(entry.url, onNavigate)}
                >
                  {entry.title}
                </a>
              </li>
            ))}
          </ul>
        </nav>
        {search}
      </div>
    </div>
  )
}

export type BlogLayoutProps = {
  navigation: ReactNode
  /** A full-width block between the navigation and the content (a category's hero). */
  banner?: ReactNode
  /** A full-width block after the content (a post's related rail). */
  after?: ReactNode
  children: ReactNode
}

export function BlogLayout({ navigation, banner, after, children }: BlogLayoutProps) {
  return (
    <div className={styles.layout}>
      {navigation}
      {banner}
      <div className={styles.content}>{children}</div>
      {after}
    </div>
  )
}
