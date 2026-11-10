// TODO(figma-ds): replace with the generated FeaturedPost component when the DCL UI library lands
import type { CSSProperties } from 'react'
import { BlogBadge } from './BlogBadge'
import { BlogLink } from './BlogRouting'
import { blogPostHref, formatBlogDate, type BlogCard } from './blogModel'
import styles from './BlogFeaturedCard.module.css'

export type BlogFeaturedCardProps = { post: BlogCard }

/** The lead post of the index: text on the amethyst panel, the post's picture beside it. */
export function BlogFeaturedCard({ post }: BlogFeaturedCardProps) {
  const date = formatBlogDate(post.publishedAt)
  return (
    <article className={styles.card}>
      <div className={styles.panel}>
        <p className={styles.meta}>
          <BlogBadge category={post.category} />
          <span>
            {date ? (
              <>
                <time dateTime={post.publishedAt}>{date}</time> ·{' '}
              </>
            ) : null}
            {post.readMinutes} min read
          </span>
        </p>
        <h2 className={styles.title}>
          <BlogLink className={styles.link} href={blogPostHref(post.slug)}>
            {post.title}
          </BlogLink>
        </h2>
        <p className={styles.excerpt}>{post.description}</p>
        {/* Decorative: the title link already covers the card, and the shared Button only renders
            as <a> or <button>, which would nest an interactive element inside that link. */}
        <span className={styles.cta} aria-hidden="true">
          Read article
        </span>
      </div>
      <div
        className={styles.image}
        style={{ '--blog-hue': post.hue } as CSSProperties}
        aria-hidden="true"
      />
    </article>
  )
}
