import { render, screen, within } from '@testing-library/react'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { BlogIndexPage, type BlogIndexPageProps } from './BlogIndexPage'
import { BlogRoutingProvider, type BlogFormProps, type BlogLinkProps } from './BlogRouting'
import { card, image } from './fixtures.test-utils'

const categories = [
  { slug: 'platform', title: 'Platform' },
  { slug: 'announcements', title: 'Announcements' }
]

// Newest first, as the loader hands them over.
const posts = [
  card({ slug: 'new', title: 'New post', publishedAt: '2026-07-02', readMinutes: 3 }),
  card({ slug: 'mid', title: 'Mid post', publishedAt: '2026-07-01' }),
  card({ slug: 'old', title: 'Old post', publishedAt: '2026-06-29' })
]

const props = (over: Partial<BlogIndexPageProps> = {}): BlogIndexPageProps => ({
  categories,
  activeCategory: '',
  title: 'The Decentraland Blog',
  subtitle: 'Stories',
  query: '',
  posts,
  total: posts.length,
  nextHref: null,
  emptyText: 'There are no articles yet.',
  ...over
})

const renderIndex = (over: Partial<BlogIndexPageProps> = {}) =>
  render(<BlogIndexPage {...props(over)} />)

describe('BlogIndexPage', () => {
  let container: HTMLElement

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when the page lists the latest posts', () => {
    beforeEach(() => {
      renderIndex()
    })

    it('should render a single h1', () => {
      expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1)
    })

    it('should feature the first post with a link to it', () => {
      const featured = screen.getByRole('heading', { level: 2, name: 'New post' })
      expect(within(featured).getByRole('link')).toHaveAttribute('href', '/blog/platform/new')
    })

    it('should show the read time of the featured post', () => {
      expect(screen.getByText(/3 min read/)).toBeInTheDocument()
    })

    it('should list the rest of the posts in order', () => {
      const latest = screen.getByRole('region', { name: 'Latest stories' })
      const titles = within(latest)
        .getAllByRole('heading', { level: 3 })
        .map((h) => h.textContent)
      expect(titles).toEqual(['Mid post', 'Old post'])
    })

    it('should not show Load more without a next page', () => {
      expect(screen.queryByRole('link', { name: 'Load more' })).not.toBeInTheDocument()
    })
  })

  describe('when the featured post has no date', () => {
    beforeEach(() => {
      renderIndex({ posts: [card({ slug: 'u', publishedAt: '', readMinutes: 4 })] })
    })

    it('should keep the read time', () => {
      expect(screen.getByText('4 min read')).toBeInTheDocument()
    })
  })

  describe('when a routing is provided', () => {
    let Link: Mock<(props: BlogLinkProps) => ReactElement>
    let Form: Mock<(props: BlogFormProps) => ReactElement>

    beforeEach(() => {
      Link = vi.fn((props: BlogLinkProps) => <a data-routed="" href={props.href} />)
      Form = vi.fn((props: BlogFormProps) => <form data-routed="" {...props} />)
      render(
        <BlogRoutingProvider routing={{ Link, Form }}>
          <BlogIndexPage {...props({ nextHref: '/blog?page=2' })} />
        </BlogRoutingProvider>
      )
    })

    it("should submit every search through the host's form", () => {
      for (const form of screen.getAllByRole('search')) expect(form).toHaveAttribute('data-routed')
    })

    it('should let the host prefetch a blog category on intent', () => {
      expect(Link.mock.calls.map(([props]) => props)).toContainEqual(
        expect.objectContaining({ href: '/blog/platform', prefetch: 'intent' })
      )
    })

    it("should load more through the host's link without resetting the scroll", () => {
      const more = Link.mock.calls.map(([p]) => p).find((p) => p.href === '/blog?page=2')
      expect(more?.preventScrollReset).toBe(true)
    })
  })

  describe('when the page changes without a remount', () => {
    beforeEach(() => {
      const view = renderIndex({ query: 'web', searched: true })
      view.rerender(<BlogIndexPage {...props()} />)
    })

    it('should bring the search field in line with the new page', () => {
      for (const box of screen.getAllByRole('searchbox')) expect(box).toHaveValue('')
    })
  })

  describe('when a category in the menu is active', () => {
    let nav: HTMLElement

    beforeEach(() => {
      renderIndex({ activeCategory: 'platform' })
      nav = screen.getByRole('navigation', { name: 'Blog categories' })
    })

    it('should link every category by path', () => {
      const links = within(nav).getAllByRole('link')
      expect(links.map((l) => [l.textContent, l.getAttribute('href')])).toEqual([
        ['All articles', '/blog'],
        ['Platform', '/blog/platform'],
        ['Announcements', '/blog/announcements']
      ])
    })

    it('should render the shared navigation bar and keep desktop search', () => {
      const links = within(nav).getAllByRole('link')
      expect(nav).toHaveAttribute('data-page', 'blog')
      expect(links.every((link) => link.hasAttribute('data-label'))).toBe(true)
      expect(within(nav).getByRole('searchbox')).toBeInTheDocument()
    })

    it('should keep the native link free of router prefetch attributes', () => {
      expect(within(nav).getByRole('link', { name: 'Platform' })).not.toHaveAttribute('prefetch')
    })

    it('should mark the active tab with aria-current', () => {
      expect(within(nav).getByRole('link', { name: 'Platform' })).toHaveAttribute(
        'aria-current',
        'page'
      )
    })
  })

  describe('when the category menu repeats a slug', () => {
    beforeEach(() => {
      renderIndex({
        categories: [...categories, { slug: 'platform', title: 'Platform duplicate' }]
      })
    })

    it('should keep the first link for each destination in menu order', () => {
      const nav = screen.getByRole('navigation', { name: 'Blog categories' })
      expect(
        within(nav)
          .getAllByRole('link')
          .map((link) => link.textContent)
      ).toEqual(['All articles', 'Platform', 'Announcements'])
    })
  })

  describe('when the category menu contains a blank slug', () => {
    beforeEach(() => {
      renderIndex({
        categories: [
          { slug: '', title: 'Duplicate home' },
          { slug: ' ', title: 'No category' }
        ]
      })
    })

    it('should render only the All articles link', () => {
      const nav = screen.getByRole('navigation', { name: 'Blog categories' })
      expect(within(nav).getByRole('link')).toHaveAccessibleName('All articles')
    })
  })

  describe('when a category title is blank', () => {
    beforeEach(() => {
      renderIndex({ categories: [{ slug: 'platform', title: '  ' }] })
    })

    it('should name the link with the category slug', () => {
      const nav = screen.getByRole('navigation', { name: 'Blog categories' })
      expect(within(nav).getByRole('link', { name: 'platform' })).toHaveAttribute(
        'href',
        '/blog/platform'
      )
    })
  })

  describe.each([
    ['an author', null],
    ['a category outside the menu', 'interview']
  ] as const)('when the page is for %s', (_, activeCategory) => {
    beforeEach(() => {
      renderIndex({ activeCategory })
    })

    it('should highlight no tab', () => {
      const nav = screen.getByRole('navigation', { name: 'Blog categories' })
      expect(within(nav).queryByRole('link', { current: 'page' })).not.toBeInTheDocument()
    })
  })

  describe('when there is a next page', () => {
    beforeEach(() => {
      renderIndex({ total: 9, nextHref: '/blog?page=2' })
    })

    it('should show Load more linking to it', () => {
      expect(screen.getByRole('link', { name: 'Load more' })).toHaveAttribute(
        'href',
        '/blog?page=2'
      )
    })
  })

  describe('when the page shows search results', () => {
    beforeEach(() => {
      renderIndex({ title: 'Search results', query: 'web', searched: true, total: 12 })
    })

    it('should count the results against the CMS total', () => {
      expect(screen.getByText('12 articles for “web”')).toBeInTheDocument()
    })

    it('should not show the latest stories region', () => {
      expect(screen.queryByRole('region', { name: 'Latest stories' })).not.toBeInTheDocument()
    })

    it('should list every result in the grid', () => {
      expect(screen.getAllByRole('heading', { level: 3 }).map((h) => h.textContent)).toEqual([
        'New post',
        'Mid post',
        'Old post'
      ])
    })

    it('should render two search forms that GET /blog/search with the query only', () => {
      const forms = screen.getAllByRole('search')
      expect(forms).toHaveLength(2)
      for (const form of forms) {
        expect(form).toHaveAttribute('action', '/blog/search')
        expect(form).toHaveAttribute('method', 'get')
        expect(form).toHaveFormValues({ q: 'web' })
      }
    })

    it('should not render a combobox', () => {
      expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
    })
  })

  describe('when there are no posts', () => {
    beforeEach(() => {
      renderIndex({ posts: [], total: 0, emptyText: 'There are no articles in this category yet.' })
    })

    it('should show the empty state heading', () => {
      expect(screen.getByRole('heading', { name: 'No articles found' })).toBeInTheDocument()
    })

    it('should show the empty state text', () => {
      expect(screen.getByText('There are no articles in this category yet.')).toBeInTheDocument()
    })

    it('should link home', () => {
      expect(screen.getByRole('link', { name: 'View all articles' })).toHaveAttribute(
        'href',
        '/blog'
      )
    })
  })

  describe('when a post has no parseable date', () => {
    beforeEach(() => {
      renderIndex({
        posts: [
          card({ slug: 'a', publishedAt: '' }),
          card({ slug: 'b', title: 'B', publishedAt: '', readMinutes: 7 })
        ]
      })
    })

    it('should omit the date', () => {
      expect(document.querySelector('time')).toBeNull()
    })

    it('should keep the read time', () => {
      expect(screen.getByText('7 min')).toBeTruthy()
    })
  })

  describe('when the posts have pictures', () => {
    let featured: HTMLImageElement
    let grid: HTMLImageElement

    beforeEach(() => {
      ;({ container } = renderIndex({
        posts: [
          card({ slug: 'a', image: image() }),
          card({ slug: 'b', title: 'B', image: image() })
        ]
      }))
      ;[featured, grid] = Array.from(container.querySelectorAll('img')) as [
        HTMLImageElement,
        HTMLImageElement
      ]
    })

    it('should load the featured picture eagerly', () => {
      expect(featured).toHaveAttribute('loading', 'eager')
    })

    it('should load the card pictures lazily', () => {
      expect(grid).toHaveAttribute('loading', 'lazy')
    })

    it('should render every picture decoratively with its width', () => {
      for (const img of [featured, grid]) {
        expect(img).toHaveAttribute('alt', '')
        expect(img).toHaveAttribute('width', '1200')
      }
    })
  })
})
