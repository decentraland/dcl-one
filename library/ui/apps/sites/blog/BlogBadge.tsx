// TODO(figma-ds): replace with the generated Badge component when the DCL UI library lands
import { blogBadgeTone, type BlogCategory } from './blogModel'
import styles from './BlogBadge.module.css'

export type BlogBadgeProps = { category: BlogCategory }

/** A post's category as a coloured pill. */
export function BlogBadge({ category }: BlogBadgeProps) {
  return (
    <span className={`${styles.badge} ${styles[blogBadgeTone(category.slug)]}`}>
      {category.title}
    </span>
  )
}
