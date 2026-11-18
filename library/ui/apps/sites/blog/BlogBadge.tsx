// TODO(figma-ds): replace with the generated Badge component when the DCL UI library lands
import { blogBadgeTone, blogCardBadgeTone, type BlogCategory } from './blogModel'
import styles from './BlogBadge.module.css'

export type BlogBadgeProps = { category: BlogCategory; card?: boolean }

/** A post's category as a coloured pill. */
export function BlogBadge({ category, card = false }: BlogBadgeProps) {
  return (
    <span
      className={`${styles.badge} ${styles[blogBadgeTone(category.slug)]}`}
      data-card-tone={card ? blogCardBadgeTone(category.slug) : undefined}
    >
      {category.title}
    </span>
  )
}
