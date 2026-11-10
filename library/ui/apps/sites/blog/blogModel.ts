// The blog's view contract and the pure helpers its components share.

export type BlogSort = 'newest' | 'oldest'

export type BlogCategory = { slug: string; title: string }

export type BlogCard = {
  slug: string
  title: string
  description: string
  /** ISO date (YYYY-MM-DD), or "" when unknown. */
  publishedAt: string
  readMinutes: number
  category: BlogCategory
  author: string
  /** Placeholder hue (0-360) shown where the post has no image. */
  hue: number
}

export type BlogRelatedCard = Pick<BlogCard, 'slug' | 'title' | 'publishedAt' | 'category' | 'hue'>

export type BlogBlock =
  | { type: 'p'; content: string }
  | { type: 'h2'; content: string }
  | { type: 'h3'; content: string }
  | { type: 'h4'; content: string }
  | { type: 'quote'; content: string }
  | { type: 'ul'; items: string[] }

export type BlogArticle = BlogCard & { body: readonly BlogBlock[] }

export const BLOG_HOME = '/blog'

/** The longest search the index reads; the input's maxLength and the loader's trim both use it. */
export const BLOG_QUERY_MAX = 200

export const blogPostHref = (slug: string) => `${BLOG_HOME}/${encodeURIComponent(slug)}`

export const blogCategoryHref = (slug: string) =>
  slug ? `${BLOG_HOME}?category=${encodeURIComponent(slug)}` : BLOG_HOME

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

/** "2026-07-02" as "Jul 02, 2026"; "" for anything that is not an ISO date. */
export function formatBlogDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso)
  if (!m) return ''
  const month = MONTHS[Number(m[2]) - 1]
  return month ? `${month} ${m[3]}, ${m[1]}` : ''
}

export type BlogShareLinks = { x: string; linkedin: string }

export const blogShareLinks = (title: string, absoluteUrl: string): BlogShareLinks => ({
  x: `https://x.com/intent/post?text=${encodeURIComponent(title)}&url=${encodeURIComponent(absoluteUrl)}`,
  linkedin: `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(absoluteUrl)}`
})

export type BlogBadgeTone = 'ruby' | 'violet' | 'orange'

const TONES: Record<string, BlogBadgeTone> = {
  announcements: 'ruby',
  platform: 'violet',
  community: 'orange'
}

export const blogBadgeTone = (slug: string): BlogBadgeTone => TONES[slug] ?? 'ruby'

/** Up to two initials of a name, upper case; "" for a blank name. */
export function blogInitials(name: string): string {
  return name
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word[0]!.toUpperCase())
    .join('')
}
