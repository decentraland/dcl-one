// TODO(figma-ds): replace with the generated Tabs component when the DCL UI library lands
import { BlogLink } from './BlogRouting'
import { BlogSearchForm } from './BlogSearchForm'
import { blogCategoryHref, type BlogCategory } from './blogModel'
import styles from './BlogTabsMenu.module.css'

export type BlogTabsMenuProps = {
  categories: readonly BlogCategory[]
  /** The highlighted tab's slug ("" is All articles); null highlights none. */
  activeCategory: string | null
  /** `page` on a list, where the tab is the page shown; `true` on a post in that category. */
  current: 'page' | 'true'
  query: string
}

/** Category tabs, then search (hidden on small screens, where the page shows its own field). */
export function BlogTabsMenu({ categories, activeCategory, current, query }: BlogTabsMenuProps) {
  const tabs = [{ slug: '', title: 'All articles' }, ...categories]
  return (
    <div className={styles.bar}>
      <nav className={styles.tabs} aria-label="Blog categories">
        <ul className={styles.list}>
          {tabs.map((tab) => {
            const active = tab.slug === activeCategory
            return (
              <li key={tab.slug || 'all'}>
                <BlogLink
                  className={active ? `${styles.tab} ${styles.active}` : styles.tab}
                  href={blogCategoryHref(tab.slug)}
                  aria-current={active ? current : undefined}
                >
                  {tab.title}
                </BlogLink>
              </li>
            )
          })}
        </ul>
      </nav>
      <div className={styles.search}>
        <BlogSearchForm variant="bar" query={query} />
      </div>
    </div>
  )
}
