import type { BlogNavigate } from './blogLinks'
import { BlogPostCard, type BlogCardPost } from './BlogPostCard'
import styles from './BlogRelatedPosts.module.css'

// The related rail under a post, as source's RelatedPost (components/blog/RelatedPost): nothing while
// loading or when there is nothing to show.

export type BlogRelatedPostsProps = {
  posts: readonly BlogCardPost[]
  loading?: boolean
  onNavigate?: BlogNavigate
}

export function BlogRelatedPosts({ posts, loading = false, onNavigate }: BlogRelatedPostsProps) {
  if (loading || posts.length === 0) return null
  return (
    <section className={styles.section}>
      <div className={styles.container}>
        <h2 className={styles.title}>Related Articles</h2>
        <div className={styles.rail}>
          {posts.map((post) => (
            <BlogPostCard key={post.id} post={post} onNavigate={onNavigate} />
          ))}
        </div>
      </div>
    </section>
  )
}
