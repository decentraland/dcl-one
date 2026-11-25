import { BlogBanner } from '../../../components/BlogBanner/BlogBanner'
import { BlogBadge } from './BlogBadge'
import { BlogCover } from './BlogCover'
import { BlogArticleLink } from './BlogRouting'
import { blogPostHref, formatBlogDate, type BlogCard } from './blogModel'

export type BlogFeaturedCardProps = { post: BlogCard }

export function BlogFeaturedCard({ post }: BlogFeaturedCardProps) {
  const date = formatBlogDate(post.publishedAt)
  return (
    <BlogBanner
      title={post.title}
      href={blogPostHref(post.category.slug, post.slug)}
      Link={BlogArticleLink}
      category={<BlogBadge category={post.category} card />}
      cover={<BlogCover image={post.image} hue={post.hue} gradient priority decorative />}
      metadata={
        <>
          {date ? (
            <>
              <time dateTime={post.publishedAt}>{date}</time> ·{' '}
            </>
          ) : null}
          {post.readMinutes} min read
        </>
      }
      summary={post.description}
      actionLabel="Read article"
    />
  )
}
