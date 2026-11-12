import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { BlogArticlePage, type BlogArticlePageProps } from './BlogArticlePage'
import { BlogNotFound, BlogUnavailable } from './BlogNotFound'
import { article, image } from './fixtures.test-utils'

const categories = [{ slug: 'platform', title: 'Platform' }]
const URL_ = 'https://sites.test/blog/post'

const renderArticle = (over: Partial<BlogArticlePageProps> = {}) =>
  render(
    <BlogArticlePage
      post={article()}
      shareUrl={URL_}
      related={[]}
      categories={categories}
      {...over}
    />
  )

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('BlogArticlePage', () => {
  it('renders the header, byline, lead and body with one h1', () => {
    renderArticle()
    expect(screen.getAllByRole('heading', { level: 1 }).map((h) => h.textContent)).toEqual([
      'A post'
    ])
    expect(screen.getByRole('link', { name: 'Back to blog' })).toHaveAttribute('href', '/blog')
    expect(screen.getByText('Jul 01, 2026')).toHaveAttribute('datetime', '2026-07-01')
    expect(screen.getByText(/2 min read/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Jane Doe' })).toHaveAttribute(
      'href',
      '/blog/author/jane-doe'
    )
    expect(screen.getByText('About the post')).toBeInTheDocument()
    expect(screen.getByText('Body text')).toBeInTheDocument()
  })

  it("highlights the post's category tab as current", () => {
    renderArticle()
    expect(screen.getByRole('link', { name: 'Platform' })).toHaveAttribute('aria-current', 'true')
  })

  it('shares on X and LinkedIn in a new tab with real share URLs', () => {
    renderArticle()
    const x = screen.getByRole('link', { name: /Share on X/ })
    expect(x.getAttribute('href')).toContain(encodeURIComponent(URL_))
    expect(x).toHaveAttribute('target', '_blank')
    expect(x).toHaveAttribute('rel', 'noopener noreferrer')
    const linkedin = screen.getByRole('link', { name: /Share on LinkedIn/ })
    expect(linkedin.getAttribute('href')).toBe(
      `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(URL_)}`
    )
  })

  it('copies the link and announces it', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    renderArticle()
    await userEvent.click(screen.getByRole('button', { name: 'Copy link' }))
    expect(writeText).toHaveBeenCalledWith(URL_)
    expect(await screen.findByRole('status')).toHaveTextContent('Link copied')
  })

  it('reports a refused clipboard write', async () => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) }
    })
    renderArticle()
    await userEvent.click(screen.getByRole('button', { name: 'Copy link' }))
    expect(await screen.findByText('Could not copy the link')).toBeInTheDocument()
  })

  it('reports a missing clipboard and clears the message after a while', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('navigator', {})
    renderArticle()
    act(() => screen.getByRole('button', { name: 'Copy link' }).click())
    expect(screen.getByRole('status')).toHaveTextContent('Could not copy the link')
    act(() => vi.advanceTimersByTime(3000))
    expect(screen.getByRole('status')).toHaveTextContent('')
  })

  it('names the related section after the category when every post shares it', () => {
    renderArticle({
      related: [
        {
          slug: 'b',
          title: 'B',
          publishedAt: '2026-06-01',
          category: categories[0]!,
          image: null,
          hue: 1
        }
      ]
    })
    const section = screen.getByRole('region', { name: 'More on Platform' })
    expect(within(section).getByRole('link', { name: 'B' })).toHaveAttribute(
      'href',
      '/blog/platform/b'
    )
  })

  it('uses a neutral related heading for mixed categories and none without related posts', () => {
    const { unmount } = renderArticle({
      related: [
        {
          slug: 'b',
          title: 'B',
          publishedAt: '',
          category: { slug: 'x', title: 'X' },
          image: null,
          hue: 1
        }
      ]
    })
    expect(screen.getByRole('region', { name: 'More from the blog' })).toBeInTheDocument()
    unmount()
    renderArticle()
    expect(screen.queryByRole('region')).not.toBeInTheDocument()
  })

  it('shows no related section when the related posts could not be read', () => {
    renderArticle({ related: null })
    expect(screen.queryByRole('region')).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('A post')
  })

  it('omits the author when the post has none, and names one without a page without a link', () => {
    const { unmount } = renderArticle({
      post: article({ author: { slug: '', name: '', image: '' } })
    })
    expect(screen.queryByText('Jane Doe')).not.toBeInTheDocument()
    unmount()
    renderArticle({ post: article({ author: { slug: '', name: 'Decentraland', image: '' } }) })
    expect(screen.getByText('Decentraland')).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Decentraland' })).not.toBeInTheDocument()
  })

  it("shows the author's picture when the CMS has one", () => {
    const { container } = renderArticle({
      post: article({
        author: { slug: 'j', name: 'Jane Doe', image: 'https://images.example/j.png' }
      })
    })
    const avatar = container.querySelector('img[src="https://images.example/j.png"]')
    expect(avatar).toHaveAttribute('alt', '')
  })

  it('renders the post picture with alt and size, eagerly, above the fold', () => {
    renderArticle({ post: article({ image: image('A plaza at night') }) })
    const hero = screen.getByRole('img', { name: 'A plaza at night' })
    expect(hero).toHaveAttribute('src', 'https://images.example/hero.png')
    expect(hero).toHaveAttribute('width', '1200')
    expect(hero).toHaveAttribute('height', '600')
    expect(hero).toHaveAttribute('loading', 'eager')
  })

  it('falls back to the hue gradient, hidden from assistive tech, without a picture', () => {
    const { container } = renderArticle({ post: article({ image: null, hue: 42 }) })
    expect(container.querySelector('article img')).toBeNull()
    const cover = container.querySelector('[style*="--blog-hue"]')
    expect(cover).toHaveAttribute('aria-hidden', 'true')
    expect((cover as HTMLElement).style.getPropertyValue('--blog-hue')).toBe('42')
  })
})

describe('BlogNotFound and BlogUnavailable', () => {
  it.each([
    ['post', 'Post not found'],
    ['category', 'Category not found'],
    ['author', 'Author not found']
  ] as const)('names a missing %s and links back to the blog', (kind, title) => {
    render(<BlogNotFound kind={kind} />)
    expect(screen.getByRole('heading', { level: 1, name: title })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Back to the Blog' })).toHaveAttribute('href', '/blog')
  })

  it('says the blog is unavailable, without any detail of why', () => {
    const { container } = render(<BlogUnavailable />)
    expect(
      screen.getByRole('heading', { level: 1, name: 'The blog is temporarily unavailable' })
    ).toBeInTheDocument()
    expect(container.textContent).not.toMatch(/https?:|cms/i)
  })
})
