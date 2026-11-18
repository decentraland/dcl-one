import { afterEach, describe, expect, it, vi } from 'vitest'
import { HttpError } from '~/lib/http'
import { buildBlogPosts, fetchLatestBlogPosts, normalizeAssetUrl, postUrl } from './blog'

const BASE = 'https://cms-api.decentraland.org/spaces/ea2ybdmmn1kv/environments/master'

const postsPayload = {
  total: 1,
  items: [
    {
      sys: { id: 'post-1' },
      fields: {
        id: 'fresh-post',
        title: 'Fresh Post',
        publishedDate: '2026-06-15T11:00-07:00',
        image: { sys: { id: 'asset-1' } },
        category: { sys: { id: 'cat-1' } }
      }
    }
  ]
}
const categoriesPayload = { items: [{ sys: { id: 'cat-1' }, fields: { id: 'announcements', title: 'Announcements' } }] }
const assetPayload = { fields: { file: { url: '//images.example.com/cover.png' } } }

const okResponse = (payload: unknown) => ({ ok: true, status: 200, json: () => Promise.resolve(payload) }) as Response
const failedResponse = { ok: false, status: 500, body: null } as Response

function stubCms(overrides: Partial<Record<'posts' | 'categories' | 'assets', () => Promise<Response>>> = {}) {
  const fetchMock = vi.fn((url: string) => {
    if (url.includes('/blog/posts')) return (overrides.posts ?? (() => Promise.resolve(okResponse(postsPayload))))()
    if (url.includes('/blog/categories'))
      return (overrides.categories ?? (() => Promise.resolve(okResponse(categoriesPayload))))()
    return (overrides.assets ?? (() => Promise.resolve(okResponse(assetPayload))))()
  })
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})

describe('fetchLatestBlogPosts', () => {
  it('joins posts, categories and cover images into cards linking to the sites blog', async () => {
    const fetchMock = stubCms()
    await expect(fetchLatestBlogPosts()).resolves.toEqual([
      {
        id: 'post-1',
        title: 'Fresh Post',
        publishedDate: '2026-06-15T11:00-07:00',
        categoryTitle: 'Announcements',
        imageUrl: 'https://images.example.com/cover.png',
        url: 'https://decentraland.zone/blog/announcements/fresh-post'
      }
    ])
    expect(fetchMock).toHaveBeenCalledWith(`${BASE}/blog/posts?limit=3`, expect.anything())
    expect(fetchMock).toHaveBeenCalledWith(`${BASE}/blog/categories`, expect.anything())
    expect(fetchMock).toHaveBeenCalledWith(`${BASE}/assets/asset-1`, expect.anything())
  })

  it('keeps a post without its cover when the asset request fails', async () => {
    stubCms({ assets: () => Promise.reject(new TypeError('Failed to fetch')) })
    const [post] = await fetchLatestBlogPosts()
    expect(post.imageUrl).toBeNull()
    expect(post.title).toBe('Fresh Post')
  })

  it('links a post to the blog search when its category is unknown', async () => {
    stubCms({ categories: () => Promise.resolve(failedResponse) })
    const [post] = await fetchLatestBlogPosts()
    expect(post.categoryTitle).toBeNull()
    expect(post.url).toBe('https://decentraland.zone/blog/search?q=Fresh%20Post')
  })

  it('resolves to no posts on an empty or malformed list', async () => {
    stubCms({ posts: () => Promise.resolve(okResponse({})) })
    await expect(fetchLatestBlogPosts()).resolves.toEqual([])
  })

  it('fails with the status when the posts request gets an error response', async () => {
    stubCms({ posts: () => Promise.resolve(failedResponse) })
    await expect(fetchLatestBlogPosts()).rejects.toEqual(expect.objectContaining({ status: 500 }))
    await expect(fetchLatestBlogPosts()).rejects.toBeInstanceOf(HttpError)
  })
})

describe('buildBlogPosts', () => {
  it('drops posts missing an id, slug or title', () => {
    const posts = [
      { sys: { id: 'p1' }, fields: { id: 'slug', title: '' } },
      { sys: { id: 'p2' }, fields: { id: '', title: 'No slug' } },
      { sys: { id: 'p3' }, fields: { id: 'ok', title: 'Ok' } }
    ]
    expect(buildBlogPosts(posts, null, new Map()).map(post => post.id)).toEqual(['p3'])
  })

  it('degrades malformed CMS items instead of throwing', () => {
    const posts = [
      null,
      'not a post',
      { sys: {}, fields: { id: 'no-id', title: 'No id' } },
      { sys: { id: 'p1' } },
      { sys: { id: 'p2' }, fields: { id: 'ok', title: 'Ok', category: {}, image: { sys: null } } }
    ]
    const categories = { items: [null, { sys: { id: 'c1' } }] } as never
    expect(buildBlogPosts(posts, categories, new Map())).toEqual([
      expect.objectContaining({
        id: 'p2',
        categoryTitle: null,
        imageUrl: null,
        url: expect.stringContaining('/blog/search?q=Ok')
      })
    ])
  })
})

describe('helpers', () => {
  it('normalizes protocol-relative asset urls', () => {
    expect(normalizeAssetUrl('//images.example.com/a.png')).toBe('https://images.example.com/a.png')
    expect(normalizeAssetUrl('https://images.example.com/a.png')).toBe('https://images.example.com/a.png')
  })

  it('builds post urls on the sites domain', () => {
    expect(postUrl({ title: 'T', slug: 's', categorySlug: 'c' })).toBe('https://decentraland.zone/blog/c/s')
    expect(postUrl({ title: 'A & B', slug: 's', categorySlug: null })).toBe(
      'https://decentraland.zone/blog/search?q=A%20%26%20B'
    )
  })
})
