import type { BlogCardProps } from '../BlogCard/BlogCard'
import linkStyles from '../BlogCard/BlogCardLink.module.css'
import styles from './BlogBanner.module.css'

export type BlogBannerProps = Omit<BlogCardProps, 'author' | 'variant'> & {
  summary?: string
  actionLabel: string
}

export function BlogBanner({
  title,
  href,
  cover,
  category,
  metadata,
  summary,
  actionLabel,
  className,
  Link = 'a'
}: BlogBannerProps) {
  return (
    <article className={[styles.banner, className].filter(Boolean).join(' ')}>
      <div className={styles.panel}>
        {(category != null || metadata != null) && (
          <div className={styles.meta}>
            {category}
            {metadata != null && <span>{metadata}</span>}
          </div>
        )}
        <h2 className={styles.title}>
          <Link className={linkStyles.link} href={href}>
            {title}
          </Link>
        </h2>
        {summary && <p className={styles.summary}>{summary}</p>}
        <span className={styles.cta} aria-hidden="true">
          {actionLabel}
        </span>
      </div>
      <div className={styles.cover}>{cover}</div>
    </article>
  )
}
