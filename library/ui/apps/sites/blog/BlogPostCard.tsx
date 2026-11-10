// TODO(figma-ds): replace with the generated PostCard component when the DCL UI library lands
import type { CSSProperties } from 'react'
import { BlogAvatar } from './BlogAvatar'
import { BlogBadge } from './BlogBadge'
import { BlogLink } from './BlogRouting'
import { blogPostHref, formatBlogDate, type BlogCard, type BlogRelatedCard } from './blogModel'
import styles from './BlogPostCard.module.css'

export type BlogPostCardProps =
  { variant?: 'grid'; post: BlogCard } | { variant: 'related'; post: BlogRelatedCard }

/** A post in the index grid or the related row. The title link covers the whole card. */
export function BlogPostCard(props: BlogPostCardProps) {
  const { post } = props
  const grid = props.variant !== 'related'
  const date = formatBlogDate(post.publishedAt)
  return (
    <article className={`${styles.card} ${grid ? styles.grid : styles.related}`}>
      <div
        className={styles.image}
        style={{ '--blog-hue': post.hue } as CSSProperties}
        aria-hidden="true"
      />
      <div className={styles.body}>
        {date ? (
          <p className={styles.meta}>
            <time dateTime={post.publishedAt}>{date}</time>
            {props.variant !== 'related' ? ` · ${props.post.readMinutes} min` : null}
          </p>
        ) : null}
        <h3 className={styles.title}>
          <BlogLink className={styles.link} href={blogPostHref(post.slug)}>
            {post.title}
          </BlogLink>
        </h3>
        <BlogBadge category={post.category} />
        {props.variant !== 'related' && props.post.author ? (
          <>
            <hr className={styles.divider} />
            <p className={styles.author}>
              <BlogAvatar name={props.post.author} />
              {props.post.author}
            </p>
          </>
        ) : null}
      </div>
    </article>
  )
}
