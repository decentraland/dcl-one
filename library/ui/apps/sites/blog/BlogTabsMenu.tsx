import type { ReactNode } from 'react'
import { TabsBar } from '../../../components/TabsBar/TabsBar'
import { BlogLink } from './BlogRouting'
import { BlogSearchForm } from './BlogSearchForm'
import { blogCategoryHref, type BlogCategory } from './blogModel'

export type BlogTabsMenuProps = {
  categories: readonly BlogCategory[]
  /** The highlighted tab's slug ("" is All articles); null highlights none. */
  activeCategory: string | null
  /** `page` on a list, where the tab is the page shown; `true` on a post in that category. */
  current: 'page' | 'true'
  query: string
  desktopSearch?: ReactNode
}

/** Category tabs, then search (hidden on small screens, where the page shows its own field). */
export function BlogTabsMenu({
  categories,
  activeCategory,
  current,
  query,
  desktopSearch
}: BlogTabsMenuProps) {
  const home = blogCategoryHref('')
  const items = new Map([[home, { id: home, href: home, label: 'All articles' }]])
  for (const { slug, title } of categories) {
    if (!slug.trim()) continue
    const href = blogCategoryHref(slug)
    if (!items.has(href)) items.set(href, { id: href, href, label: title.trim() || slug })
  }
  return (
    <TabsBar
      mode="navigation"
      page="blog"
      label="Blog categories"
      items={[...items.values()]}
      activeId={activeCategory === null ? null : blogCategoryHref(activeCategory)}
      current={current}
      LinkComponent={BlogLink}
      search={desktopSearch ?? <BlogSearchForm variant="bar" query={query} />}
    />
  )
}
