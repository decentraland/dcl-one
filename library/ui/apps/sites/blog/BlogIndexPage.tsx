// TODO(figma-ds): replace with the generated BlogIndex template when the DCL UI library lands
import { useId } from 'react'
import { BlogFeaturedCard } from './BlogFeaturedCard'
import { BlogPostCard } from './BlogPostCard'
import { BlogLink } from './BlogRouting'
import { BlogSearchForm } from './BlogSearchForm'
import { BlogShell } from './BlogShell'
import { BlogTabsMenu } from './BlogTabsMenu'
import { BLOG_HOME, type BlogCard, type BlogCategory } from './blogModel'
import styles from './BlogIndexPage.module.css'

export type BlogIndexPageProps = {
  /** The categories the tabs show. */
  categories: readonly BlogCategory[]
  /** The highlighted tab ("" is All articles); null highlights none (an author or a search). */
  activeCategory: string | null
  title: string
  subtitle: string
  /** The search shown in the field; with `searched`, the page is its results. */
  query: string
  searched?: boolean
  /** The posts loaded so far, newest first (search: best match first). */
  posts: readonly BlogCard[]
  /** Every post the list has, loaded or not. */
  total: number
  /** The link that loads the next page, or null at the end. */
  nextHref: string | null
  /** Shown when there are no posts. */
  emptyText: string
}

/** A list of posts (the home, a category, an author or a search): hero, a featured post, the grid, "Load more". */
export function BlogIndexPage({
  categories,
  activeCategory,
  title,
  subtitle,
  query,
  searched,
  posts,
  total,
  nextHref,
  emptyText
}: BlogIndexPageProps) {
  const latestId = useId()
  const featured = searched ? undefined : posts[0]
  const rest = searched ? posts : posts.slice(1)
  // A category outside the tabs (one the CMS hides from the menu) highlights none.
  const active =
    activeCategory && !categories.some((c) => c.slug === activeCategory) ? null : activeCategory
  return (
    <BlogShell>
      <BlogTabsMenu categories={categories} activeCategory={active} current="page" query={query} />
      <div className={styles.inner}>
        <header className={styles.hero}>
          <h1 className={styles.title}>{title}</h1>
          {subtitle ? <p className={styles.subtitle}>{subtitle}</p> : null}
        </header>
        <div className={styles.mobileSearch}>
          <BlogSearchForm variant="outline" query={query} />
        </div>
        {searched && posts.length ? (
          <p className={styles.summary}>
            {total === 1 ? '1 article' : `${total} articles`} for “{query}”
          </p>
        ) : null}
        {posts.length ? (
          <>
            {featured ? <BlogFeaturedCard post={featured} /> : null}
            {rest.length ? (
              <section className={featured ? styles.latest : undefined} aria-labelledby={latestId}>
                <h2 className={featured ? styles.heading : styles.srOnly} id={latestId}>
                  {featured ? 'Latest stories' : 'Results'}
                </h2>
                <ul className={styles.grid}>
                  {rest.map((post) => (
                    <li key={post.slug}>
                      <BlogPostCard post={post} />
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
            {nextHref ? (
              <div className={styles.more}>
                <BlogLink className={styles.action} href={nextHref} preventScrollReset>
                  Load more
                </BlogLink>
              </div>
            ) : null}
          </>
        ) : (
          <div className={styles.empty}>
            <h2 className={styles.heading}>No articles found</h2>
            <p className={styles.emptyText}>{emptyText}</p>
            <BlogLink className={styles.action} href={BLOG_HOME}>
              View all articles
            </BlogLink>
          </div>
        )}
      </div>
    </BlogShell>
  )
}
