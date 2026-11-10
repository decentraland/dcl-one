// TODO(figma-ds): replace with the generated BlogIndex template when the DCL UI library lands
import { useId } from 'react'
import { BlogFeaturedCard } from './BlogFeaturedCard'
import { BlogPostCard } from './BlogPostCard'
import { BlogLink } from './BlogRouting'
import { BlogSearchForm } from './BlogSearchForm'
import { BlogShell } from './BlogShell'
import { BlogTabsMenu } from './BlogTabsMenu'
import { BLOG_HOME, type BlogCard, type BlogCategory, type BlogSort } from './blogModel'
import styles from './BlogIndexPage.module.css'

export type BlogIndexPageProps = {
  categories: readonly BlogCategory[]
  /** The `?category=` slug; "" for all posts. */
  category: string
  query: string
  sort: BlogSort
  /** The posts that match, already in `sort` order. */
  posts: readonly BlogCard[]
}

/** The blog index: hero, the first matching post (in `sort` order) featured, then the rest in a grid. */
export function BlogIndexPage({ categories, category, query, sort, posts }: BlogIndexPageProps) {
  const latestId = useId()
  const [featured, ...rest] = posts
  const known = !category || categories.some((c) => c.slug === category)
  return (
    <BlogShell>
      <BlogTabsMenu
        categories={categories}
        activeCategory={known ? category : null}
        current="page"
        query={query}
        sort={sort}
        searchCategory={category}
      />
      <div className={styles.inner}>
        <header className={styles.hero}>
          <h1 className={styles.title}>The Decentraland Blog</h1>
          <p className={styles.subtitle}>
            Stories, guides and things to do in a world built by you.
          </p>
        </header>
        <div className={styles.mobileSearch}>
          <BlogSearchForm variant="outline" query={query} sort={sort} category={category} />
        </div>
        {query ? (
          <p className={styles.summary}>
            {posts.length === 1 ? '1 article' : `${posts.length} articles`} for “{query}”
          </p>
        ) : null}
        {featured ? (
          <>
            <BlogFeaturedCard post={featured} />
            {rest.length ? (
              <section className={styles.latest} aria-labelledby={latestId}>
                <h2 className={styles.heading} id={latestId}>
                  Latest stories
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
          </>
        ) : (
          <div className={styles.empty}>
            <h2 className={styles.heading}>No articles found</h2>
            <p className={styles.emptyText}>
              {query
                ? `Nothing matches “${query}”. Try another word or browse every article.`
                : 'There are no articles in this category yet.'}
            </p>
            <BlogLink className={styles.emptyLink} href={BLOG_HOME}>
              View all articles
            </BlogLink>
          </div>
        )}
      </div>
    </BlogShell>
  )
}
