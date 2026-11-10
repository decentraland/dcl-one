import { useEffect, useRef } from 'react'
import { formatBlogDate, inAppLink, type BlogNavigate } from './blogLinks'
import { BlogPostCard, BlogPostCardSkeleton, type BlogCardPost } from './BlogPostCard'
import styles from './BlogPostGrid.module.css'

// A post list, as source's PostList and MainPostCard (components/blog/PostList, MainPostCard): a grid
// of cards, the first post as a wide hero on desktop, skeletons while loading, and more posts as the
// reader nears the end (source's infinite scroll). The hero is chosen by CSS, not by a media query in
// script, so the server render and the hydrated one are the same: the hero shows from the shared
// desktop breakpoint (1024px, source 1096px, plan R1-3) and the first post's card hides there.

export type BlogMainPost = BlogCardPost & { description: string }

export type BlogPostGridProps = {
  posts: readonly BlogMainPost[]
  /** Show the first post as the hero on desktop (home and category lists; not author lists). */
  hasMainPost?: boolean
  /** The first page is loading and nothing is shown yet. */
  loading?: boolean
  /** A next page is loading: its skeletons follow the cards. */
  loadingMore?: boolean
  /** Called once the reader nears the end of the list, while more posts exist. */
  onEndReached?: () => void
  hasMore?: boolean
  /** The list failed to load: source's error copy in its place. */
  error?: boolean
  onNavigate?: BlogNavigate
}

const FIRST_COUNT = 7
const NEXT_COUNT = 6

export function BlogMainPostCard({
  post,
  onNavigate
}: {
  post: BlogMainPost
  onNavigate?: BlogNavigate
}) {
  return (
    <div className={styles.main}>
      <a className={styles.mainImageLink} href={post.url} {...inAppLink(post.url, onNavigate)}>
        <img
          className={styles.mainImage}
          src={post.image.url}
          alt={post.title}
          width={697}
          height={349}
          fetchPriority="high"
          decoding="async"
        />
      </a>
      <div>
        <div>
          <span className={styles.mainDate}>{formatBlogDate(post.publishedDate)}</span>
          <span>
            <a
              className={styles.mainCategory}
              href={post.category.url}
              {...inAppLink(post.category.url, onNavigate)}
            >
              {post.category.title}
            </a>
          </span>
        </div>
        <a className={styles.mainTitleLink} href={post.url} {...inAppLink(post.url, onNavigate)}>
          <h2 className={styles.mainTitle}>{post.title}</h2>
        </a>
        <p className={styles.mainDescription}>{post.description}</p>
      </div>
    </div>
  )
}

export function BlogMainPostCardSkeleton() {
  return (
    <div className={styles.main} aria-hidden="true">
      <div className={`${styles.bone} ${styles.mainBoneImage}`} />
      <div>
        <div className={styles.boneMeta}>
          <div className={`${styles.bone} ${styles.boneShort}`} />
          <div className={`${styles.bone} ${styles.boneShort}`} />
        </div>
        <div className={`${styles.bone} ${styles.boneTitle}`} />
        <div className={`${styles.bone} ${styles.boneTitle} ${styles.boneTitleShort}`} />
        <div className={styles.boneBody}>
          <div className={`${styles.bone} ${styles.boneLine}`} />
          <div className={`${styles.bone} ${styles.boneLine}`} />
          <div className={`${styles.bone} ${styles.boneLine}`} />
          <div className={`${styles.bone} ${styles.boneLine} ${styles.boneLineShort}`} />
        </div>
      </div>
    </div>
  )
}

/** Calls onReached when the end of the list comes near the viewport. */
function EndSentinel({ onReached }: { onReached: () => void }) {
  const ref = useRef<HTMLDivElement>(null)
  const latest = useRef(onReached)
  latest.current = onReached
  useEffect(() => {
    const element = ref.current
    if (!element || typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) latest.current()
      },
      { rootMargin: '0px 0px 400px 0px' }
    )
    observer.observe(element)
    return () => observer.disconnect()
  }, [])
  return <div ref={ref} className={styles.sentinel} aria-hidden="true" />
}

export function BlogPostGrid({
  posts,
  hasMainPost = false,
  loading = false,
  loadingMore = false,
  onEndReached,
  hasMore = false,
  error = false,
  onNavigate
}: BlogPostGridProps) {
  if (error)
    return (
      <p className={styles.error} role="alert">
        Failed to load posts. Please try again later.
      </p>
    )
  if (loading && posts.length === 0) {
    const cards = hasMainPost ? FIRST_COUNT - 1 : NEXT_COUNT
    return (
      <div className={styles.grid}>
        {hasMainPost ? <BlogMainPostCardSkeleton /> : null}
        {Array.from({ length: cards }, (_, index) => (
          <div key={index} className={styles.cell}>
            <BlogPostCardSkeleton layout="grid" />
          </div>
        ))}
      </div>
    )
  }
  if (posts.length === 0) return null
  const [first] = posts
  return (
    <>
      <div className={styles.grid}>
        {hasMainPost && first ? <BlogMainPostCard post={first} onNavigate={onNavigate} /> : null}
        {posts.map((post, index) => (
          <div
            key={post.id}
            className={`${styles.cell} ${hasMainPost && index === 0 ? styles.belowDesktop : ''}`}
            // The hero shows this post on desktop; this card stands in for it below.
            data-blog-hero-twin={hasMainPost && index === 0 ? '' : undefined}
          >
            <BlogPostCard post={post} onNavigate={onNavigate} layout="grid" />
          </div>
        ))}
        {loadingMore
          ? Array.from({ length: NEXT_COUNT }, (_, index) => (
              <div key={`more-${index}`} className={styles.cell}>
                <BlogPostCardSkeleton layout="grid" />
              </div>
            ))
          : null}
      </div>
      {hasMore && !loadingMore && onEndReached ? <EndSentinel onReached={onEndReached} /> : null}
    </>
  )
}
