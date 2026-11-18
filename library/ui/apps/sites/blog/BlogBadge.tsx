// TODO(figma-ds): replace with the generated Badge component when the DCL UI library lands
import { blogBadgeTone, type BlogCategory } from './blogModel'
import styles from './BlogBadge.module.css'

export type BlogBadgeProps = { category: BlogCategory; card?: boolean }

const cardTones: Record<string, 'updates' | 'communities' | 'about'> = {
  announcements: 'updates',
  updates: 'updates',
  community: 'communities',
  communities: 'communities',
  'community-highlights': 'communities',
  'about-decentraland': 'about',
  platform: 'about'
}

/** A post's category as a coloured pill. */
export function BlogBadge({ category, card = false }: BlogBadgeProps) {
  return (
    <span
      className={`${styles.badge} ${styles[blogBadgeTone(category.slug)]}`}
      data-card-tone={card ? (cardTones[category.slug] ?? 'about') : undefined}
    >
      {category.title}
    </span>
  )
}
