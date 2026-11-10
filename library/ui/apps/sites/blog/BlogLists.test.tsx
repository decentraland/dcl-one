import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { BlogAuthorHeader, BlogCategoryHero } from './BlogListHeaders'
import { BlogPostGrid, type BlogMainPost } from './BlogPostGrid'
import { BlogSearchResults, type BlogSearchResultsProps } from './BlogSearchResults'

const post = (n: number): BlogMainPost => ({
  id: `p${n}`,
  url: `/blog/announcements/p${n}`,
  title: `Post ${n}`,
  description: `About ${n}`,
  publishedDate: '2020-08-25T00:00:00Z',
  image: { url: `https://cms.example/p${n}.png` },
  category: { title: 'Announcements', url: '/blog/announcements' }
})

// jsdom has no IntersectionObserver: record the observed end and let the test reach it.
let reach: (() => void) | null = null
class FakeObserver {
  constructor(private callback: IntersectionObserverCallback) {}
  observe(target: Element) {
    reach = () =>
      this.callback(
        [{ isIntersecting: true, target } as IntersectionObserverEntry],
        this as unknown as IntersectionObserver
      )
  }
  disconnect() {
    reach = null
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
  reach = null
})

describe('BlogPostGrid', () => {
  it('shows the first post as the hero and keeps its card for narrower viewports, marked as the twin', () => {
    const root = render(<BlogPostGrid posts={[1, 2, 3].map(post)} hasMainPost />).container
    const titles = [...root.querySelectorAll('a h2')].map((h2) => h2.textContent)
    expect(titles).toEqual(['Post 1', 'Post 1', 'Post 2', 'Post 3'])
    expect(root.querySelectorAll('[data-blog-hero-twin]')).toHaveLength(1)
    expect(root.querySelector('[data-blog-hero-twin] h2')?.textContent).toBe('Post 1')
    // The hero carries the standfirst and loads eagerly.
    expect(screen.getByText('About 1').tagName).toBe('P')
    expect(root.querySelector('img[fetchpriority="high"]')?.getAttribute('alt')).toBe('Post 1')
  })

  it('has no hero and no twin on lists without one (author)', () => {
    const root = render(<BlogPostGrid posts={[1, 2].map(post)} />).container
    expect(root.querySelectorAll('a h2')).toHaveLength(2)
    expect(root.querySelector('[data-blog-hero-twin]')).toBeNull()
  })

  it.each([
    [true, 7],
    [false, 6]
  ])('shows skeletons while the first page loads (hero %s: %i)', (hasMainPost, count) => {
    const root = render(<BlogPostGrid posts={[]} loading hasMainPost={hasMainPost} />).container
    expect(root.querySelectorAll('[aria-hidden="true"]')).toHaveLength(count)
  })

  it('shows the source error copy, as an alert, in place of the list', () => {
    render(<BlogPostGrid posts={[post(1)]} error />)
    expect(screen.getByRole('alert').textContent).toBe(
      'Failed to load posts. Please try again later.'
    )
    expect(screen.queryByRole('heading')).toBeNull()
  })

  it('renders nothing for an empty list', () => {
    expect(render(<BlogPostGrid posts={[]} />).container.innerHTML).toBe('')
  })

  it('adds 6 skeletons while the next page loads, and asks for more only near the end, with more to load', () => {
    vi.stubGlobal('IntersectionObserver', FakeObserver)
    const onEndReached = vi.fn()
    const { container, rerender } = render(
      <BlogPostGrid posts={[1, 2].map(post)} hasMore onEndReached={onEndReached} />
    )
    expect(onEndReached).not.toHaveBeenCalled()
    act(() => reach!())
    expect(onEndReached).toHaveBeenCalledTimes(1)
    rerender(
      <BlogPostGrid posts={[1, 2].map(post)} hasMore loadingMore onEndReached={onEndReached} />
    )
    expect(container.querySelectorAll('[aria-hidden="true"]')).toHaveLength(6)
    expect(reach).toBeNull()
    rerender(<BlogPostGrid posts={[1, 2].map(post)} hasMore={false} onEndReached={onEndReached} />)
    expect(reach).toBeNull()
  })

  it('follows its links in the app', () => {
    const onNavigate = vi.fn()
    const root = render(
      <BlogPostGrid posts={[post(1)]} hasMainPost onNavigate={onNavigate} />
    ).container
    fireEvent.click(root.querySelector('a[href="/blog/announcements"]')!)
    expect(onNavigate).toHaveBeenCalledWith('/blog/announcements')
  })
})

describe('list headers', () => {
  it('renders the category hero as source: an h3 and its description over the shaded image', () => {
    const root = render(
      <BlogCategoryHero
        title="Announcements"
        description="News"
        image="https://cms.example/c.png"
      />
    ).container
    expect(root.querySelector('h3')?.textContent).toBe('Announcements')
    expect(root.querySelector('p')?.textContent).toBe('News')
    expect((root.firstElementChild as HTMLElement).style.backgroundImage).toContain(
      'https://cms.example/c.png'
    )
  })

  it('drops an unsafe category image', () => {
    const root = render(
      <BlogCategoryHero title="A" description="" image="javascript:alert(1)" />
    ).container
    expect((root.firstElementChild as HTMLElement).style.backgroundImage).toBe('')
  })

  it('renders the author header as source: image, an h5 and the description', () => {
    const root = render(
      <BlogAuthorHeader name="Ariel" description="Builder" image="https://cms.example/a.png" />
    ).container
    expect([...root.firstElementChild!.children].map((child) => child.tagName)).toEqual([
      'IMG',
      'H5',
      'P'
    ])
    expect(root.querySelector('img')?.getAttribute('alt')).toBe('Ariel')
  })

  it('leaves the image out for an author without one', () => {
    expect(
      render(<BlogAuthorHeader name="Ariel" description="" />).container.querySelector('img')
    ).toBeNull()
  })
})

describe('BlogSearchResults', () => {
  const hit = (n: number) => ({
    id: `p${n}`,
    url: `/blog/announcements/p${n}`,
    image: `https://cms.example/p${n}.png`,
    highlightedTitle: `About <em>Wearables</em> ${n}`,
    highlightedDescription: 'A <em>wearable</em> contest'
  })
  const results = (props: Partial<BlogSearchResultsProps>) =>
    render(
      <BlogSearchResults
        q="wearables"
        searchable
        hits={[]}
        loading={false}
        loadingMore={false}
        hasMore={false}
        onLoadMore={() => undefined}
        {...props}
      />
    ).container

  it('heads the results with the query, as source', () => {
    expect(results({ hits: [hit(1)] }).querySelector('p')?.textContent).toBe(
      'Search results for “wearables”'
    )
  })

  it('renders each hit as a link with a decorative image and its highlighted title and description', () => {
    const root = results({ hits: [hit(1)] })
    const link = root.querySelector('a[href="/blog/announcements/p1"]')!
    expect([...link.querySelectorAll(':scope img, :scope p')].map((el) => el.tagName)).toEqual([
      'IMG',
      'P',
      'P'
    ])
    expect(link.querySelector('img')?.getAttribute('alt')).toBe('')
    expect([...link.querySelectorAll('em')].map((em) => em.textContent)).toEqual([
      'Wearables',
      'wearable'
    ])
  })

  it('offers Load More only with more to load and nothing loading', () => {
    const onLoadMore = vi.fn()
    results({ hits: [hit(1)], hasMore: true, onLoadMore })
    fireEvent.click(screen.getByRole('button', { name: 'Load More' }))
    expect(onLoadMore).toHaveBeenCalledTimes(1)
  })

  it('moves keyboard focus to the first new hit once more hits arrive after Load More', () => {
    const props = {
      q: 'wearables',
      searchable: true,
      loading: false,
      hasMore: true,
      onLoadMore: vi.fn()
    }
    const { rerender, container } = render(
      <BlogSearchResults {...props} hits={[hit(1)]} loadingMore={false} />
    )
    const button = screen.getByRole('button', { name: 'Load More' })
    button.focus()
    fireEvent.click(button)
    rerender(<BlogSearchResults {...props} hits={[hit(1)]} loadingMore />)
    rerender(<BlogSearchResults {...props} hits={[hit(1), hit(2)]} loadingMore={false} />)
    expect(document.activeElement).toBe(container.querySelector('a[href="/blog/announcements/p2"]'))
  })

  it('leaves focus alone when Load More was clicked without focus (mouse in some browsers)', () => {
    const props = {
      q: 'wearables',
      searchable: true,
      loading: false,
      hasMore: true,
      onLoadMore: vi.fn()
    }
    const { rerender } = render(
      <BlogSearchResults {...props} hits={[hit(1)]} loadingMore={false} />
    )
    fireEvent.click(screen.getByRole('button', { name: 'Load More' }))
    rerender(<BlogSearchResults {...props} hits={[hit(1), hit(2)]} loadingMore={false} />)
    expect(document.activeElement).toBe(document.body)
  })

  it.each([
    ['loading the first page', { loading: true }, 5],
    ['loading more', { hits: [hit(1)], loadingMore: true }, 3]
  ] as const)('shows skeletons while %s', (_name, props, count) => {
    const root = results(props)
    expect(root.querySelectorAll('[aria-hidden="true"]')).toHaveLength(count)
    expect(screen.queryByRole('button', { name: 'Load More' })).toBeNull()
  })

  it('says there is nothing to show for a query without hits', () => {
    expect(results({}).textContent).toContain('Nothing to showNo results found for “wearables”')
  })

  it('shows no heading and no empty state for a query too short to search', () => {
    expect(results({ q: 'ab', searchable: false }).textContent).toBe('')
  })
})
