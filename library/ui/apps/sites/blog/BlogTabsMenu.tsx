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
  const items = [{ slug: '', title: 'All articles' }, ...categories].map(({ slug, title }) => {
    const href = blogCategoryHref(slug)
    return { id: href, href, label: title }
  })
  return (
    <TabsBar
      mode="navigation"
      page="blog"
      label="Blog categories"
      items={items}
      activeId={activeCategory === null ? null : blogCategoryHref(activeCategory)}
      current={current}
      LinkComponent={BlogLink}
      search={desktopSearch ?? <BlogSearchForm variant="bar" query={query} />}
    />
  )
}
