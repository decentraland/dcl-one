import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { BlogIndexPage, type BlogIndexPageProps } from './BlogIndexPage'
import { card } from './fixtures.test-utils'

const categories = [
  { slug: 'platform', title: 'Platform' },
  { slug: 'announcements', title: 'Announcements' }
]

const posts = [
  card({ slug: 'old', title: 'Old post', publishedAt: '2026-06-29' }),
  card({ slug: 'new', title: 'New post', publishedAt: '2026-07-02', readMinutes: 3 }),
  card({ slug: 'mid', title: 'Mid post', publishedAt: '2026-07-01' })
]

const renderIndex = (over: Partial<BlogIndexPageProps> = {}) =>
  render(
    <BlogIndexPage
      categories={categories}
      category=""
      query=""
      sort="newest"
      posts={posts}
      {...over}
    />
  )

describe('BlogIndexPage', () => {
  it('renders one h1, features the newest post and lists the rest in order', () => {
    renderIndex()
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1)
    const featured = screen.getByRole('heading', { level: 2, name: 'New post' })
    expect(within(featured).getByRole('link')).toHaveAttribute('href', '/blog/new')
    expect(screen.getByText(/3 min read/)).toBeInTheDocument()
    const latest = screen.getByRole('region', { name: 'Latest stories' })
    const titles = within(latest)
      .getAllByRole('heading', { level: 3 })
      .map((h) => h.textContent)
    expect(titles).toEqual(['Old post', 'Mid post'])
  })

  it('marks the active tab with aria-current and links every category', () => {
    renderIndex({ category: 'platform', posts: [posts[0]!] })
    const nav = screen.getByRole('navigation', { name: 'Blog categories' })
    const links = within(nav).getAllByRole('link')
    expect(links.map((l) => [l.textContent, l.getAttribute('href')])).toEqual([
      ['All articles', '/blog'],
      ['Platform', '/blog?category=platform'],
      ['Announcements', '/blog?category=announcements']
    ])
    expect(within(nav).getByRole('link', { name: 'Platform' })).toHaveAttribute(
      'aria-current',
      'page'
    )
    expect(within(nav).getByRole('link', { name: 'All articles' })).not.toHaveAttribute(
      'aria-current'
    )
  })

  it('a single post renders featured only, without the Latest stories section', () => {
    renderIndex({ query: 'old', posts: [posts[0]!] })
    expect(screen.getByText('1 article for “old”')).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: 'Latest stories' })).not.toBeInTheDocument()
  })

  it('an unknown category highlights no tab and shows the empty state', () => {
    renderIndex({ category: 'nope', posts: [] })
    const nav = screen.getByRole('navigation', { name: 'Blog categories' })
    expect(within(nav).queryByRole('link', { current: 'page' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'No articles found' })).toBeInTheDocument()
    expect(screen.getByText('There are no articles in this category yet.')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'View all articles' })).toHaveAttribute('href', '/blog')
  })

  it('a search with no match says so and keeps the query in the field', () => {
    renderIndex({ query: 'xyz', posts: [] })
    expect(screen.getByText('0 articles for “xyz”')).toBeInTheDocument()
    expect(screen.getByText(/Nothing matches “xyz”/)).toBeInTheDocument()
    for (const box of screen.getAllByRole('searchbox', { name: 'Search articles' }))
      expect(box).toHaveValue('xyz')
  })

  it('search forms GET /blog and carry the category and a non-default sort', () => {
    renderIndex({ category: 'platform', sort: 'oldest', query: 'web' })
    const forms = screen.getAllByRole('search')
    expect(forms).toHaveLength(2)
    for (const form of forms) {
      expect(form).toHaveAttribute('action', '/blog')
      expect(form).toHaveAttribute('method', 'get')
      expect(form).toHaveFormValues({ q: 'web', category: 'platform', sort: 'oldest' })
    }
    expect(screen.getByText('3 articles for “web”')).toBeInTheDocument()
  })

  it('the sort menu shows the current sort and submits its form on change', async () => {
    renderIndex({ sort: 'oldest' })
    const select = screen.getByRole('combobox', { name: 'Sort:' })
    expect(select).toHaveValue('oldest')
    const form = select.closest('form')!
    const submit = vi.fn((event: SubmitEvent) => event.preventDefault())
    form.addEventListener('submit', submit)
    await userEvent.selectOptions(select, 'newest')
    expect(submit).toHaveBeenCalledTimes(1)
  })

  it('omits the date line when a post has no parseable date', () => {
    renderIndex({
      posts: [
        card({ slug: 'a', publishedAt: '' }),
        card({ slug: 'b', title: 'B', publishedAt: '' })
      ]
    })
    expect(document.querySelector('time')).toBeNull()
  })
})
