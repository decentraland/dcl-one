import { BlogCard as ArticleCard } from '../../../components/BlogCard/BlogCard'
import { ProfilePic } from '../../../components/ProfilePic/ProfilePic'
import { BlogBadge } from './BlogBadge'
import { BlogCover } from './BlogCover'
import { BlogArticleLink } from './BlogRouting'
import { blogPostHref, formatBlogDate, type BlogCard, type BlogRelatedCard } from './blogModel'

export type BlogPostCardProps =
  { variant?: 'grid'; post: BlogCard } | { variant: 'related'; post: BlogRelatedCard }

export function BlogPostCard(props: BlogPostCardProps) {
  const { post } = props
  const related = props.variant === 'related'
  const date = formatBlogDate(post.publishedAt)
  return (
    <ArticleCard
      variant={related ? 'related' : 'standard'}
      title={post.title}
      href={blogPostHref(post.category.slug, post.slug)}
      Link={BlogArticleLink}
      cover={<BlogCover image={post.image} hue={post.hue} gradient decorative />}
      category={<BlogBadge category={post.category} card={!related} />}
      metadata={
        date || !related ? (
          <>
            {date ? <time dateTime={post.publishedAt}>{date}</time> : null}
            {date && !related ? ' · ' : null}
            {!related ? `${props.post.readMinutes} min` : null}
          </>
        ) : null
      }
      author={
        !related && props.post.author.name ? (
          <>
            <ProfilePic
              name={props.post.author.name}
              src={props.post.author.image}
              alt=""
              size={28}
              fallback="gradient"
              loading="lazy"
            />
            {props.post.author.name}
          </>
        ) : null
      }
    />
  )
}
