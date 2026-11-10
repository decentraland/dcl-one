import { formatBlogDate, inAppLink, type BlogNavigate } from './blogLinks'
import styles from './BlogPostCard.module.css'

// A post in a grid or the related rail, as source's PostCard (components/blog/PostCard).

/** The post fields a card shows. */
export type BlogCardPost = {
  id: string
  url: string
  title: string
  /** Raw ISO 8601 value. */
  publishedDate: string
  image: { url: string }
  category: { title: string; url: string }
}

export type BlogPostCardProps = {
  post: BlogCardPost
  onNavigate?: BlogNavigate
  /** `rail` sizes itself in a flex row (source widths); `grid` fills a grid cell. */
  layout?: 'rail' | 'grid'
}

export function BlogPostCard({ post, onNavigate, layout = 'rail' }: BlogPostCardProps) {
  return (
    <div className={layout === 'grid' ? styles.gridCard : styles.card}>
      <a className={styles.imageLink} href={post.url} {...inAppLink(post.url, onNavigate)}>
        <img
          className={styles.image}
          src={post.image.url}
          alt={post.title}
          loading="lazy"
          decoding="async"
        />
      </a>
      <div>
        <div>
          <span className={styles.date}>{formatBlogDate(post.publishedDate)}</span>
          <span>
            <a
              className={styles.category}
              href={post.category.url}
              {...inAppLink(post.category.url, onNavigate)}
            >
              {post.category.title}
            </a>
          </span>
        </div>
        <a className={styles.titleLink} href={post.url} {...inAppLink(post.url, onNavigate)}>
          <h2 className={styles.title}>{post.title}</h2>
        </a>
      </div>
    </div>
  )
}

/** The card's loading state (source PostCard `loading`). */
export function BlogPostCardSkeleton({ layout = 'rail' }: { layout?: 'rail' | 'grid' }) {
  return (
    <div className={layout === 'grid' ? styles.gridCard : styles.card} aria-hidden="true">
      <div className={`${styles.bone} ${styles.boneImage}`} />
      <div className={styles.boneMeta}>
        <div className={`${styles.bone} ${styles.boneShort}`} />
        <div className={`${styles.bone} ${styles.boneShort}`} />
      </div>
      <div className={`${styles.bone} ${styles.boneLine}`} />
      <div className={`${styles.bone} ${styles.boneLine} ${styles.boneLineShort}`} />
    </div>
  )
}
