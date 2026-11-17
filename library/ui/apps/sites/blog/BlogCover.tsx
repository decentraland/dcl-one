import type { CSSProperties } from 'react'
import type { BlogImage } from './blogModel'
import styles from './BlogCover.module.css'

export type BlogCoverProps = {
  /** Sizes the cover and paints the hue gradient behind it (the no-image fallback). */
  className?: string
  image: BlogImage | null
  hue: number
  /** Above the fold: loaded at once. Otherwise lazily. */
  priority?: boolean
  /** Next to a link that already names the post: the picture adds nothing for a screen reader. */
  decorative?: boolean
}

/** A post's picture, or its hue gradient when the CMS has none. */
export function BlogCover({ className, image, hue, priority, decorative }: BlogCoverProps) {
  const alt = decorative || !image ? '' : image.alt
  return (
    <div
      className={className ? `${styles.cover} ${className}` : styles.cover}
      style={{ '--blog-hue': hue } as CSSProperties}
      aria-hidden={alt ? undefined : true}
    >
      {image ? (
        <img
          className={styles.img}
          src={image.url}
          alt={alt}
          width={image.width || undefined}
          height={image.height || undefined}
          loading={priority ? 'eager' : 'lazy'}
          decoding="async"
        />
      ) : null}
    </div>
  )
}
