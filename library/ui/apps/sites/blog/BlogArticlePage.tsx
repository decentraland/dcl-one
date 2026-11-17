// TODO(figma-ds): replace with the generated BlogArticle template when the DCL UI library lands
import { useId } from 'react'
import { GeneralLeftArrow } from '../../../foundations/icons/general'
import { BlogAvatar } from './BlogAvatar'
import { BlogBadge } from './BlogBadge'
import { BlogCover } from './BlogCover'
import { BlogPostCard } from './BlogPostCard'
import { BlogRichText } from './BlogRichText'
import { BlogLink } from './BlogRouting'
import { BlogShare } from './BlogShare'
import { BlogShell } from './BlogShell'
import { BlogTabsMenu } from './BlogTabsMenu'
import {
  BLOG_HOME,
  blogAuthorHref,
  formatBlogDate,
  type BlogArticle,
  type BlogCategory,
  type BlogRelatedCard
} from './blogModel'
import styles from './BlogArticlePage.module.css'

export type BlogArticlePageProps = {
  post: BlogArticle
  /** The post's absolute public URL, for sharing. */
  shareUrl: string
  /** Null when they could not be read: like an empty list, no related section. */
  related: readonly BlogRelatedCard[] | null
  categories: readonly BlogCategory[]
}

/** One post: header, byline and share, the hero, the standfirst, the body, then related posts. */
export function BlogArticlePage({ post, shareUrl, related, categories }: BlogArticlePageProps) {
  const relatedId = useId()
  const date = formatBlogDate(post.publishedAt)
  const sameCategory = related?.every((r) => r.category.slug === post.category.slug)
  return (
    <BlogShell>
      <BlogTabsMenu
        categories={categories}
        activeCategory={post.category.slug}
        current="true"
        query=""
      />
      <article className={styles.article}>
        <BlogLink className={styles.back} href={BLOG_HOME}>
          <GeneralLeftArrow size={20} aria-hidden="true" />
          Back to blog
        </BlogLink>
        <p className={styles.meta}>
          <BlogBadge category={post.category} />
          <span>
            {date ? (
              <>
                <time dateTime={post.publishedAt}>{date}</time> ·{' '}
              </>
            ) : null}
            {post.readMinutes} min read
          </span>
        </p>
        <h1 className={styles.title}>{post.title}</h1>
        <div className={styles.byline}>
          {post.author.name ? (
            <p className={styles.author}>
              <BlogAvatar name={post.author.name} image={post.author.image} size="md" />
              {post.author.slug ? (
                <BlogLink className={styles.authorName} href={blogAuthorHref(post.author.slug)}>
                  {post.author.name}
                </BlogLink>
              ) : (
                <span className={styles.authorName}>{post.author.name}</span>
              )}
            </p>
          ) : null}
          <div className={styles.share}>
            <BlogShare title={post.title} url={shareUrl} />
          </div>
        </div>
        <BlogCover className={styles.hero} image={post.image} hue={post.hue} priority />
        <p className={styles.lead}>{post.description}</p>
        <BlogRichText body={post.body} assets={post.assets} />
      </article>
      {related?.length ? (
        <section className={styles.related} aria-labelledby={relatedId}>
          <h2 className={styles.relatedTitle} id={relatedId}>
            {sameCategory ? `More on ${post.category.title}` : 'More from the blog'}
          </h2>
          <ul className={styles.relatedList}>
            {related.map((card) => (
              <li key={card.slug}>
                <BlogPostCard variant="related" post={card} />
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </BlogShell>
  )
}
