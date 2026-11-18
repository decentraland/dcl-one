import type { ComponentType, ReactNode } from 'react'
import styles from './BlogCard.module.css'

export type BlogCardLinkProps = { className?: string; href: string; children: ReactNode }

export type BlogCardProps = {
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
      className={[styles.card, className].filter(Boolean).join(' ')}
      data-blog-card-layout=""
    >
      <div className={styles.cover}>{cover}</div>
      <div className={styles.body}>
        {metadata != null && <p className={styles.meta}>{metadata}</p>}
        <h3 className={styles.title}>
          <Link className={styles.link} href={href}>
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
