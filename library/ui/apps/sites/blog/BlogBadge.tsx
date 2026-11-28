import { Tag } from '../../../components/Tag/Tag'
import { blogBadgeTone, blogCardBadgeTone, type BlogCategory } from './blogModel'

export type BlogBadgeProps = { category: BlogCategory; card?: boolean }

/** A post's category as a coloured pill: the category colours on a card, a larger brand pill elsewhere. */
export function BlogBadge({ category, card = false }: BlogBadgeProps) {
  return card ? (
    <Tag variant="category" tone={blogCardBadgeTone(category.slug)}>
      {category.title}
    </Tag>
  ) : (
    <Tag variant="category" size="lg" tone={blogBadgeTone(category.slug)}>
      {category.title}
    </Tag>
  )
}
