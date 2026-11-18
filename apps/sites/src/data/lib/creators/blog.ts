// Read-only client for the latest blog posts on the Contentful-backed CMS API ("Fresh from the blog").
import { config } from '~/config'
import { HttpError } from '~/lib/http'

type CMSLink = { sys: { id: string } }

export type CMSPostItem = {
  sys: { id: string }
  fields: {
    id: string
    title: string
    publishedDate?: string
    image?: CMSLink
    category?: CMSLink
  }
}

export type CMSPostsResponse = { items: CMSPostItem[]; total?: number }

export type CMSCategoryItem = { sys: { id: string }; fields: { id: string; title: string } }

export type CMSCategoriesResponse = { items: CMSCategoryItem[] }

export type CMSAssetResponse = { fields: { file: { url: string } } }

export type BlogPost = {
  id: string
  title: string
  publishedDate: string | null
  categoryTitle: string | null
  imageUrl: string | null
  url: string
}

const POSTS_LIMIT = 3
const FETCH_TIMEOUT_MS = 10_000

const cmsUrl = () =>
  `${config.get('CMS_API_URL')}/spaces/${config.get('CONTENTFUL_SPACE_ID')}/environments/${config.get('CONTENTFUL_ENVIRONMENT')}`

export const blogUrl = () => `${config.get('SITES_URL')}/blog`

/** A post's page; posts without a category have no page of their own, so they link to the blog search. */
export function postUrl(post: { title: string; slug: string; categorySlug: string | null }): string {
  return post.categorySlug
    ? `${blogUrl()}/${post.categorySlug}/${post.slug}`
    : `${blogUrl()}/search?q=${encodeURIComponent(post.title)}`
}

// Contentful serves asset files behind protocol-relative URLs.
export const normalizeAssetUrl = (url: string) => (url.startsWith('//') ? `https:${url}` : url)

async function fetchJson<T>(url: string, signal: AbortSignal): Promise<T> {
  const response = await fetch(url, { signal })
  if (!response.ok) {
    await response.body?.cancel()
    throw new HttpError('blog request failed', response.status)
  }
  return (await response.json()) as T
}

// A category list or cover that fails only degrades the card; the posts request is what react-query reports.
const orNull = <T>(request: Promise<T>): Promise<T | null> => request.catch(() => null)

// The CMS JSON is only type-asserted, so every nested field is narrowed before use: a malformed item is
// dropped (or loses its category/cover) instead of throwing and blanking the whole rail.
const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null
const stringOf = (value: unknown) => (typeof value === 'string' && value ? value : undefined)
const sysId = (value: unknown) => (isRecord(value) && isRecord(value.sys) ? stringOf(value.sys.id) : undefined)
const listOf = (response: unknown): unknown[] =>
  isRecord(response) && Array.isArray(response.items) ? response.items : []

/** The cover asset id of each post that has one. */
const coverAssetIds = (posts: unknown[]): string[] =>
  posts.flatMap(post => {
    const id = isRecord(post) && isRecord(post.fields) ? sysId(post.fields.image) : undefined
    return id ? [id] : []
  })

/** Joins posts with their category slug/title and resolved cover image. Posts missing an id, slug or title are dropped. */
export function buildBlogPosts(
  posts: unknown[],
  categories: CMSCategoriesResponse | null,
  assetUrlById: Map<string, string>
): BlogPost[] {
  const categoryById = new Map<string, { slug: string; title: string }>()
  for (const category of listOf(categories)) {
    const id = sysId(category)
    const fields = isRecord(category) && isRecord(category.fields) ? category.fields : undefined
    const slug = stringOf(fields?.id)
    if (id && slug) categoryById.set(id, { slug, title: stringOf(fields?.title) ?? '' })
  }
  const built: BlogPost[] = []
  for (const post of posts) {
    const id = sysId(post)
    const fields = isRecord(post) && isRecord(post.fields) ? post.fields : undefined
    const slug = stringOf(fields?.id)
    const title = stringOf(fields?.title)
    if (!id || !slug || !title) continue
    const categoryId = sysId(fields?.category)
    const category = categoryId ? categoryById.get(categoryId) : undefined
    const assetId = sysId(fields?.image)
    const rawImageUrl = assetId ? assetUrlById.get(assetId) : undefined
    built.push({
      id,
      title,
      publishedDate: stringOf(fields?.publishedDate) ?? null,
      categoryTitle: category?.title || null,
      imageUrl: rawImageUrl ? normalizeAssetUrl(rawImageUrl) : null,
      url: postUrl({ title, slug, categorySlug: category?.slug ?? null })
    })
  }
  return built
}

/** The three newest posts. A failing posts request throws; a missing category list or image degrades the card instead. */
export async function fetchLatestBlogPosts(): Promise<BlogPost[]> {
  const signal = AbortSignal.timeout(FETCH_TIMEOUT_MS)
  const base = cmsUrl()
  const [posts, categories] = await Promise.all([
    fetchJson<CMSPostsResponse>(`${base}/blog/posts?limit=${POSTS_LIMIT}`, signal),
    orNull(fetchJson<CMSCategoriesResponse>(`${base}/blog/categories`, signal))
  ])
  const items = listOf(posts)
  if (items.length === 0) return []

  const assetIds = coverAssetIds(items)
  const assets = await Promise.all(
    assetIds.map(id => orNull(fetchJson<CMSAssetResponse>(`${base}/assets/${id}`, signal)))
  )
  const assetUrlById = new Map<string, string>()
  assetIds.forEach((id, index) => {
    const asset: unknown = assets[index]
    const file =
      isRecord(asset) && isRecord(asset.fields) && isRecord(asset.fields.file) ? asset.fields.file : undefined
    const fileUrl = stringOf(file?.url)
    if (fileUrl) assetUrlById.set(id, fileUrl)
  })
  return buildBlogPosts(items, categories, assetUrlById)
}
