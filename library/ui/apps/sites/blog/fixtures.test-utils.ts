import type { BlogArticle, BlogCard } from './blogModel'

export const card = (over: Partial<BlogCard> = {}): BlogCard => ({
  slug: 'post',
  title: 'A post',
  description: 'About the post',
  publishedAt: '2026-07-01',
  readMinutes: 2,
  category: { slug: 'platform', title: 'Platform' },
  author: 'Jane Doe',
  hue: 200,
  ...over
})

export const article = (over: Partial<BlogArticle> = {}): BlogArticle => ({
  ...card(),
  body: [{ type: 'p', content: 'Body text' }],
  ...over
})
