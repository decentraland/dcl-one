// TODO(figma-ds): replace with the generated PostCard component when the DCL UI library lands
import { BlogAvatar } from './BlogAvatar'
import { BlogBadge } from './BlogBadge'
import { BlogCover } from './BlogCover'
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
      <BlogCover className={styles.image} image={post.image} hue={post.hue} decorative />
      <div className={styles.body}>
        {date || grid ? (
          <p className={styles.meta}>
            {date ? <time dateTime={post.publishedAt}>{date}</time> : null}
            {date && grid ? ' · ' : null}
            {props.variant !== 'related' ? `${props.post.readMinutes} min` : null}
          </p>
        ) : null}
        <h3 className={styles.title}>
          <BlogLink
            className={styles.link}
            href={blogPostHref(post.category.slug, post.slug)}
            data-blog-card=""
          >
            {post.title}
          </BlogLink>
        </h3>
        <BlogBadge category={post.category} />
        {props.variant !== 'related' && props.post.author.name ? (
          <>
            <hr className={styles.divider} />
            <p className={styles.author}>
              <BlogAvatar name={props.post.author.name} image={props.post.author.image} />
              {props.post.author.name}
            </p>
          </>
        ) : null}
      </div>
    </article>
  )
}
