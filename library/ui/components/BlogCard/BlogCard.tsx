import type { ComponentType, ReactNode } from 'react'
import styles from './BlogCard.module.css'
import linkStyles from './BlogCardLink.module.css'

export type BlogCardLinkProps = { className?: string; href: string; children: ReactNode }

export type BlogCardProps = {
  variant?: 'standard' | 'related'
  title: string
  href: string
  cover?: ReactNode
  category?: ReactNode
  metadata?: ReactNode
  author?: ReactNode
  className?: string
  Link?: ComponentType<BlogCardLinkProps> | 'a'
}

export function BlogCard({
  variant = 'standard',
  title,
  href,
  cover,
  category,
  metadata,
  author,
  className,
  Link = 'a'
}: BlogCardProps) {
  return (
    <article
      className={[styles.card, variant === 'related' && styles.related, className]
        .filter(Boolean)
        .join(' ')}
    >
      <div className={styles.cover}>{cover}</div>
      <div className={styles.body}>
        {metadata != null && <p className={styles.meta}>{metadata}</p>}
        <h3 className={styles.title}>
          <Link className={`${linkStyles.link} ${styles.emphasis}`} href={href}>
            {title}
          </Link>
        </h3>
        {category}
        {author != null && (
          <>
            <hr className={styles.divider} />
            <p className={styles.author}>{author}</p>
          </>
        )}
      </div>
    </article>
  )
}
