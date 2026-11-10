// TODO(figma-ds): replace with the generated FeaturedPost component when the DCL UI library lands
import type { CSSProperties } from 'react'
import { BlogBadge } from './BlogBadge'
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
          {date ? (
            <span>
              <time dateTime={post.publishedAt}>{date}</time> · {post.readMinutes} min read
            </span>
          ) : null}
        </p>
        <h2 className={styles.title}>
          <a className={styles.link} href={blogPostHref(post.slug)}>
            {post.title}
          </a>
        </h2>
        <p className={styles.excerpt}>{post.description}</p>
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
