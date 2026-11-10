import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { BlogPostArticle, type BlogArticlePost } from './BlogPostArticle'
import { BlogPostCard, BlogPostCardSkeleton, type BlogCardPost } from './BlogPostCard'
import { BlogRelatedPosts } from './BlogRelatedPosts'

const post: BlogArticlePost = {
  title: 'A post',
  description: 'Its standfirst',
  publishedDate: '2023-04-06T00:00+02:00',
  image: { url: 'https://cms.example/hero.png', width: 1200, height: 630 },
  category: { title: 'Announcements', url: '/blog/announcements' },
  author: {
    title: 'Decentraland',
    url: '/blog/author/decentraland',
    image: { url: 'https://cms.example/avatar.png' }
  }
}
const SHARE = 'https://public.example/blog/announcements/a-post'

const article = (overrides: Partial<BlogArticlePost> = {}, onNavigate?: (href: string) => void) =>
  render(
    <BlogPostArticle post={{ ...post, ...overrides }} shareUrl={SHARE} onNavigate={onNavigate}>
      <p>Body</p>
    </BlogPostArticle>
  ).container

describe('BlogPostArticle', () => {
  it('renders the hero with the title as alt, eagerly', () => {
    const hero = article().querySelector('article > img')!
    expect([
      hero.getAttribute('src'),
      hero.getAttribute('alt'),
      hero.getAttribute('fetchpriority'),
      hero.getAttribute('width')
    ]).toEqual(['https://cms.example/hero.png', 'A post', 'high', '1200'])
  })

  it('renders date • category, the only h1, the standfirst and the body, in order', () => {
    const root = article()
    expect(root.querySelector('time')?.getAttribute('dateTime')).toBe('2023-04-06T00:00+02:00')
    expect(root.querySelector('time')?.textContent).toBe('Apr 05, 2023')
    expect(root.querySelectorAll('h1')).toHaveLength(1)
    expect(root.textContent).toBe(
      'Apr 05, 2023•AnnouncementsA postIts standfirstDecentralandShareBody'
    )
  })

  it('drops the date and its separator for a dateless post', () => {
    const root = article({ publishedDate: '' })
    expect(root.querySelector('time')).toBeNull()
    expect(root.textContent).not.toContain('•')
  })

  it('shares the given public URL on X and Facebook, in a new tab without opener, with labels', () => {
    article()
    const x = screen.getByRole('link', { name: 'Share on X' })
    const facebook = screen.getByRole('link', { name: 'Share on Facebook' })
    expect(x.getAttribute('href')).toBe(
      `https://x.com/intent/post?text=A%20post&url=${encodeURIComponent(SHARE)}`
    )
    expect(facebook.getAttribute('href')).toBe(
      `https://www.facebook.com/sharer/sharer.php?u=${encodeURIComponent(SHARE)}`
    )
    for (const link of [x, facebook])
      expect([link.getAttribute('target'), link.getAttribute('rel')]).toEqual([
        '_blank',
        'noopener noreferrer'
      ])
  })

  it('hides the author row, share links included, for a post without a named author (as source)', () => {
    const root = article({ author: { title: '', url: '/blog/author/x' } })
    expect(root.querySelector('a[href="/blog/author/x"]')).toBeNull()
    expect(root.textContent).not.toContain('Share')
  })

  it('shows the author without an avatar when the author has no image', () => {
    const root = article({ author: { title: 'Someone', url: '/blog/author/someone' } })
    expect(root.querySelector('a[href="/blog/author/someone"] img')).toBeNull()
    expect(root.querySelector('a[href="/blog/author/someone"]')?.textContent).toBe('Someone')
  })

  it('follows the category and author links in the app', () => {
    const onNavigate = vi.fn()
    const root = article({}, onNavigate)
    fireEvent.click(root.querySelector('a[href="/blog/announcements"]')!)
    fireEvent.click(root.querySelector('a[href="/blog/author/decentraland"]')!)
    expect(onNavigate.mock.calls).toEqual([['/blog/announcements'], ['/blog/author/decentraland']])
  })
})

const card = (id: string): BlogCardPost => ({
  id,
  url: `/blog/announcements/${id}`,
  title: `Post ${id}`,
  publishedDate: '2020-08-25T00:00:00Z',
  image: { url: `https://cms.example/${id}.png` },
  category: { title: 'Announcements', url: '/blog/announcements' }
})

describe('BlogPostCard', () => {
  it('renders the source card: image link, date, category link, titled link, the title an h2', () => {
    const root = render(<BlogPostCard post={card('a')} />).container
    const links = [...root.querySelectorAll('a')].map((a) => a.getAttribute('href'))
    expect(links).toEqual(['/blog/announcements/a', '/blog/announcements', '/blog/announcements/a'])
    expect(root.querySelector('a img')?.getAttribute('alt')).toBe('Post a')
    expect(root.querySelector('a h2')?.textContent).toBe('Post a')
    expect(root.textContent).toBe('Aug 25, 2020AnnouncementsPost a')
  })

  it('has a loading state hidden from assistive technology', () => {
    expect(
      render(<BlogPostCardSkeleton />).container.firstElementChild?.getAttribute('aria-hidden')
    ).toBe('true')
  })
})

describe('BlogRelatedPosts', () => {
  it('renders "Related Articles" as an h2 and one card per post', () => {
    const root = render(<BlogRelatedPosts posts={[card('a'), card('b')]} />).container
    expect(root.querySelector('section > div > h2')?.textContent).toBe('Related Articles')
    expect(root.querySelectorAll('a h2')).toHaveLength(2)
  })

  it.each([
    ['loading', { posts: [card('a')], loading: true }],
    ['empty', { posts: [] }]
  ])('renders nothing when %s', (_name, props) => {
    expect(render(<BlogRelatedPosts {...props} />).container.innerHTML).toBe('')
  })
})
