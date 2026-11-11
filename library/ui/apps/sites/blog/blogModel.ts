// The blog's view contract and the pure helpers its components share.

export type BlogCategory = { slug: string; title: string }

/** A picture from the CMS; `alt` is "" when the CMS gives no title for it. */
export type BlogImage = { url: string; width: number; height: number; alt: string }

export type BlogAuthor = {
  slug: string
  name: string
  /** The author's picture URL, or "" for none (initials are shown instead). */
  image: string
}

export type BlogCard = {
  slug: string
  title: string
  description: string
  /** ISO date (YYYY-MM-DD), or "" when unknown. */
  publishedAt: string
  readMinutes: number
  category: BlogCategory
  author: BlogAuthor
  image: BlogImage | null
  /** Placeholder hue (0-360) shown where the post has no image. */
  hue: number
}

export type BlogRelatedCard = Pick<
  BlogCard,
  'slug' | 'title' | 'publishedAt' | 'category' | 'image' | 'hue'
>

/** A Contentful rich-text node, as the CMS returns it (document, block, inline or text). */
export type BlogRichTextNode = {
  nodeType: string
  data?: Record<string, unknown>
  content?: BlogRichTextNode[]
  value?: string
  marks?: { type: string }[]
}

export type BlogArticle = BlogCard & {
  body: BlogRichTextNode | null
  /** The pictures the body embeds, by CMS asset id. */
  assets: Readonly<Record<string, BlogImage>>
}

export const BLOG_HOME = '/blog'
export const BLOG_SEARCH = '/blog/search'

/** The search dropdown's suggestions endpoint (`?q=`); it answers BlogSearchSuggestions. */
export const BLOG_SEARCH_SUGGESTIONS_API = '/api/blog/search'
/** How many suggestions the dropdown shows; "See more results" opens the full search. */
export const BLOG_SEARCH_SUGGESTIONS_LIMIT = 4

/** One dropdown row: a post's card fields plus the URL it opens. */
export type BlogSearchSuggestion = Pick<
  BlogCard,
  'slug' | 'title' | 'description' | 'image' | 'hue'
> & {
  href: string
}

/** The suggestions for `query`: the first matches and how many the full search has. */
export type BlogSearchSuggestions = {
  query: string
  posts: BlogSearchSuggestion[]
  total: number
}

/** The longest search the blog reads; the input's maxLength and the loader's trim both use it. */
export const BLOG_QUERY_MAX = 200

const segment = encodeURIComponent

export const blogPostHref = (categorySlug: string, slug: string) =>
  `${BLOG_HOME}/${segment(categorySlug)}/${segment(slug)}`

export const blogCategoryHref = (slug: string) =>
  slug ? `${BLOG_HOME}/${segment(slug)}` : BLOG_HOME

export const blogAuthorHref = (slug: string) => `${BLOG_HOME}/author/${segment(slug)}`

export const blogSearchHref = (query: string) => `${BLOG_SEARCH}?q=${encodeURIComponent(query)}`

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
  community: 'orange',
  'community-highlights': 'orange'
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
