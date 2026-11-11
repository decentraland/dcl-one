import type { BlogArticle, BlogCard } from './blogModel'

export const card = (over: Partial<BlogCard> = {}): BlogCard => ({
  slug: 'post',
  title: 'A post',
  description: 'About the post',
  publishedAt: '2026-07-01',
  readMinutes: 2,
  category: { slug: 'platform', title: 'Platform' },
  author: { slug: 'jane-doe', name: 'Jane Doe', image: '' },
  image: null,
  hue: 200,
  ...over
})

export const article = (over: Partial<BlogArticle> = {}): BlogArticle => ({
  ...card(),
  body: {
    nodeType: 'document',
    content: [{ nodeType: 'paragraph', content: [{ nodeType: 'text', value: 'Body text' }] }]
  },
  assets: {},
  ...over
})

export const image = (alt = 'A plaza') => ({
  url: 'https://images.example/hero.png',
  width: 1200,
  height: 600,
  alt
})
