import type { ReactNode } from 'react'
import { SocialFacebook, SocialX } from '../../../foundations/icons'
import { formatBlogDate, inAppLink, shareLinks, type BlogNavigate } from './blogLinks'
import styles from './BlogPostArticle.module.css'

// One post, as source's PostPage renders it (pages/blog/PostPage.tsx with components/blog/PostHeader):
// hero image, date • category, title, standfirst, the author with the share links, then the body.

export type BlogArticlePost = {
  title: string
  description: string
  /** Raw ISO 8601 value. */
  publishedDate: string
  image: { url: string; width?: number; height?: number }
  category: { title: string; url: string }
  author: { title: string; url: string; image?: { url: string } }
}

export type BlogPostArticleProps = {
  post: BlogArticlePost
  /** The post's absolute public URL, for the share links. */
  shareUrl: string
  onNavigate?: BlogNavigate
  /** The rendered body. */
  children: ReactNode
}

export function BlogPostArticle({ post, shareUrl, onNavigate, children }: BlogPostArticleProps) {
  const date = formatBlogDate(post.publishedDate)
  const { author } = post
  const share = shareLinks(post.title, shareUrl)
  return (
    <article className={styles.article}>
      <img
        className={styles.hero}
        src={post.image.url}
        alt={post.title}
        width={post.image.width}
        height={post.image.height}
        fetchPriority="high"
        decoding="async"
      />
      <div className={styles.header}>
        <span className={styles.meta}>
          {/* As source: a dateless post drops the element and its separator (an empty datetime is invalid). */}
          {date ? (
            <>
              <time dateTime={post.publishedDate}>{date}</time>
              <span className={styles.separator}>•</span>
            </>
          ) : null}
          <a
            className={styles.categoryLink}
            href={post.category.url}
            {...inAppLink(post.category.url, onNavigate)}
          >
            {post.category.title}
          </a>
        </span>
        <div className={styles.titleBox}>
          <h1 className={styles.title}>{post.title}</h1>
        </div>
        <p className={styles.subtitle}>{post.description}</p>
      </div>
      {/* As source: the author row, and the share links in it, show only for a post with a named author. */}
      {author.title ? (
        <div className={styles.authorRow}>
          <div>
            <a
              className={styles.authorLink}
              href={author.url}
              {...inAppLink(author.url, onNavigate)}
            >
              {author.image?.url ? (
                <img
                  className={styles.avatar}
                  src={author.image.url}
                  alt={author.title}
                  loading="lazy"
                  decoding="async"
                />
              ) : null}
              <p className={styles.authorName}>{author.title}</p>
            </a>
          </div>
          <div className={styles.share}>
            <p className={styles.shareLabel}>Share</p>
            <a
              className={styles.shareLink}
              href={share.x}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Share on X"
            >
              <SocialX size={20} aria-hidden="true" />
            </a>
            <a
              className={styles.shareLink}
              href={share.facebook}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Share on Facebook"
            >
              <SocialFacebook size={20} aria-hidden="true" />
            </a>
          </div>
        </div>
      ) : null}
      <div>{children}</div>
    </article>
  )
}

/** The post's loading state: hero, meta, title, standfirst and a few body lines, hidden from assistive technology. */
export function BlogPostArticleSkeleton() {
  return (
    <article className={styles.article} aria-hidden="true">
      <div className={`${styles.bone} ${styles.boneHero}`} />
      <div className={styles.header}>
        <div className={`${styles.bone} ${styles.boneMeta}`} />
        <div className={`${styles.bone} ${styles.boneTitle}`} />
        <div className={`${styles.bone} ${styles.boneLine}`} />
      </div>
      <div className={`${styles.bone} ${styles.boneLine}`} />
      <div className={`${styles.bone} ${styles.boneLine}`} />
      <div className={`${styles.bone} ${styles.boneLine} ${styles.boneLineShort}`} />
    </article>
  )
}
