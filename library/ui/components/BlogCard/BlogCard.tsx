import type { ComponentType, ReactNode } from 'react'
import { Skeleton } from '../Skeleton/Skeleton'
import styles from './BlogCard.module.css'
import linkStyles from './BlogCardLink.module.css'

export type BlogCardLinkProps = { className?: string; href: string; children: ReactNode }

type BlogCardContent = {
  variant?: 'standard' | 'related'
  cover?: ReactNode
  category?: ReactNode
  metadata?: ReactNode
  author?: ReactNode
  className?: string
  Link?: ComponentType<BlogCardLinkProps> | 'a'
}

export type BlogCardProps = BlogCardContent &
  (
    | { loading?: false; title: string; href: string; loadingLabel?: never }
    | {
        /**
         * A skeleton in place of the post, marked busy: the cover, the date, a two-line title, the category and, in
         * the standard variant, the divider and the author, each in the box the loaded card gives it.
         */
        loading: true
        /** The name assistive technology reads for the loading card. */
        loadingLabel?: string
        title?: string
        href?: string
      }
  )

export function BlogCard(props: BlogCardProps) {
  const { variant = 'standard', cover, category, metadata, author, className, Link = 'a' } = props
  if (props.loading) {
    // Neutral grey, the skeleton decentraland.org's blog draws on its dark page.
    const line = (extra?: string) => (
      <Skeleton tone="neutral" className={[styles.skeletonLine, extra].filter(Boolean).join(' ')} />
    )
    return (
      <article
        className={[
          styles.card,
          variant === 'related' && styles.related,
          styles.loading,
          className
        ]
          .filter(Boolean)
          .join(' ')}
        role="progressbar"
        aria-label={props.loadingLabel ?? 'Loading'}
        aria-busy="true"
      >
        <div className={styles.cover}>
          <Skeleton tone="neutral" className={styles.skeletonCover} />
        </div>
        <div className={styles.body}>
          <p className={styles.meta}>{line(styles.skeletonShort)}</p>
          <h3 className={styles.title}>
            {line()}
            {line(styles.skeletonShort)}
          </h3>
          <span className={styles.skeletonCategory}>{line()}</span>
          {variant === 'standard' ? (
            <>
              <hr className={styles.divider} />
              <p className={styles.author}>{line(styles.skeletonShort)}</p>
            </>
          ) : null}
        </div>
      </article>
    )
  }
  const { title, href } = props
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
