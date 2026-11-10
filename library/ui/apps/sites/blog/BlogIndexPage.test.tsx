import { render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
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
  it('renders one h1, features the first post and lists the rest in order', () => {
    renderIndex()
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1)
    const featured = screen.getByRole('heading', { level: 2, name: 'New post' })
    expect(within(featured).getByRole('link')).toHaveAttribute('href', '/blog/platform/new')
    expect(screen.getByText(/3 min read/)).toBeInTheDocument()
    const latest = screen.getByRole('region', { name: 'Latest stories' })
    const titles = within(latest)
      .getAllByRole('heading', { level: 3 })
      .map((h) => h.textContent)
    expect(titles).toEqual(['Mid post', 'Old post'])
  })

  it('keeps the read time on a featured post with no date', () => {
    renderIndex({ posts: [card({ slug: 'u', publishedAt: '', readMinutes: 4 })] })
    expect(screen.getByText('4 min read')).toBeInTheDocument()
  })

  it("navigates through the host's link and form when a routing is provided", () => {
    const Link = vi.fn((props: BlogLinkProps) => <a data-routed="" href={props.href} />)
    const Form = vi.fn((props: BlogFormProps) => <form data-routed="" {...props} />)
    render(
      <BlogRoutingProvider routing={{ Link, Form }}>
        <BlogIndexPage {...props({ nextHref: '/blog?page=2' })} />
      </BlogRoutingProvider>
    )
    for (const form of screen.getAllByRole('search')) expect(form).toHaveAttribute('data-routed')
    const more = Link.mock.calls.map(([p]) => p).find((p) => p.href === '/blog?page=2')
    expect(more?.preventScrollReset).toBe(true)
  })

  it('brings the search field in line when the page changes without a remount', () => {
    const view = renderIndex({ query: 'web', searched: true })
    view.rerender(<BlogIndexPage {...props()} />)
    for (const box of screen.getAllByRole('searchbox')) expect(box).toHaveValue('')
  })

  it('marks the active tab with aria-current and links every category by path', () => {
    renderIndex({ activeCategory: 'platform' })
    const nav = screen.getByRole('navigation', { name: 'Blog categories' })
    const links = within(nav).getAllByRole('link')
    expect(links.map((l) => [l.textContent, l.getAttribute('href')])).toEqual([
      ['All articles', '/blog'],
      ['Platform', '/blog/platform'],
      ['Announcements', '/blog/announcements']
    ])
    expect(within(nav).getByRole('link', { name: 'Platform' })).toHaveAttribute(
      'aria-current',
      'page'
    )
  })

  it('highlights no tab for an author, or a category outside the menu', () => {
    for (const activeCategory of [null, 'interview']) {
      const { unmount } = renderIndex({ activeCategory })
      const nav = screen.getByRole('navigation', { name: 'Blog categories' })
      expect(within(nav).queryByRole('link', { current: 'page' })).not.toBeInTheDocument()
      unmount()
    }
  })

  it('shows Load more only while there is a next page', () => {
    const { unmount } = renderIndex({ total: 9, nextHref: '/blog?page=2' })
    expect(screen.getByRole('link', { name: 'Load more' })).toHaveAttribute('href', '/blog?page=2')
    unmount()
    renderIndex()
    expect(screen.queryByRole('link', { name: 'Load more' })).not.toBeInTheDocument()
  })

  it('lists search results in the grid, counted against the CMS total', () => {
    renderIndex({ title: 'Search results', query: 'web', searched: true, total: 12 })
    expect(screen.getByText('12 articles for “web”')).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: 'Latest stories' })).not.toBeInTheDocument()
    expect(screen.getAllByRole('heading', { level: 3 }).map((h) => h.textContent)).toEqual([
      'New post',
      'Mid post',
      'Old post'
    ])
  })

  it('shows the empty state with its text and a link home', () => {
    renderIndex({ posts: [], total: 0, emptyText: 'There are no articles in this category yet.' })
    expect(screen.getByRole('heading', { name: 'No articles found' })).toBeInTheDocument()
    expect(screen.getByText('There are no articles in this category yet.')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'View all articles' })).toHaveAttribute('href', '/blog')
  })

  it('search forms GET /blog/search with the query only', () => {
    renderIndex({ query: 'web', searched: true })
    const forms = screen.getAllByRole('search')
    expect(forms).toHaveLength(2)
    for (const form of forms) {
      expect(form).toHaveAttribute('action', '/blog/search')
      expect(form).toHaveAttribute('method', 'get')
      expect(form).toHaveFormValues({ q: 'web' })
    }
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument()
  })

  it('omits the date but keeps the read time when a post has no parseable date', () => {
    renderIndex({
      posts: [
        card({ slug: 'a', publishedAt: '' }),
        card({ slug: 'b', title: 'B', publishedAt: '', readMinutes: 7 })
      ]
    })
    expect(document.querySelector('time')).toBeNull()
    expect(screen.getByText('7 min')).toBeTruthy()
  })

  it('renders card pictures lazily and decoratively, the featured one eagerly', () => {
    const { container } = renderIndex({
      posts: [card({ slug: 'a', image: image() }), card({ slug: 'b', title: 'B', image: image() })]
    })
    const [featured, grid] = Array.from(container.querySelectorAll('img'))
    expect(featured).toHaveAttribute('loading', 'eager')
    expect(grid).toHaveAttribute('loading', 'lazy')
    for (const img of [featured, grid]) {
      expect(img).toHaveAttribute('alt', '')
      expect(img).toHaveAttribute('width', '1200')
    }
  })
})
