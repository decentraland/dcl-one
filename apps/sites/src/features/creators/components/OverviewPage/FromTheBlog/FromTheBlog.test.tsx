import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import type { BlogPost } from '~/lib/blog'
import { FromTheBlog } from './FromTheBlog'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: true }) }))

const cms = vi.hoisted(() => ({ data: undefined as BlogPost[] | undefined }))
vi.mock('~/hooks/useLatestBlogPosts', () => ({ useLatestBlogPosts: () => ({ data: cms.data }) }))

const aPost = (overrides: Partial<BlogPost>): BlogPost => ({
  id: 'post-1',
  title: 'A Post',
  publishedDate: '2026-06-15T12:00:00Z',
  categoryTitle: 'Announcements',
  imageUrl: 'https://img.example.com/cover.png',
  url: 'https://decentraland.zone/blog/announcements/a-post',
  ...overrides
})

beforeEach(() => {
  cms.data = undefined
  vi.mocked(track).mockClear()
})

const renderSection = () =>
  render(
    <TranslationProvider>
      <FromTheBlog />
    </TranslationProvider>
  )

describe('FromTheBlog', () => {
  it('renders nothing while loading or without posts', () => {
    const { container } = renderSection()
    expect(container).toBeEmptyDOMElement()
    cms.data = []
    expect(renderSection().container).toBeEmptyDOMElement()
  })

  it('links each post to the blog, showing the cover and category only when present', () => {
    cms.data = [
      aPost({}),
      aPost({
        id: 'p2',
        title: 'No Image Post',
        imageUrl: null,
        categoryTitle: null,
        url: 'https://decentraland.zone/blog/search?q=No%20Image%20Post'
      })
    ]
    renderSection()
    const cards = screen.getAllByTestId('overview-blog-card')
    expect(cards.map(card => card.getAttribute('href'))).toEqual([
      'https://decentraland.zone/blog/announcements/a-post',
      'https://decentraland.zone/blog/search?q=No%20Image%20Post'
    ])
    expect(cards[0]).toHaveTextContent('Announcements')
    expect(cards[0]).toHaveTextContent('June 15, 2026')
    expect(cards[0].querySelector('img')).not.toBeNull()
    expect(cards[1].querySelector('img')).toBeNull()
    expect(screen.getByTestId('overview-blog-all')).toHaveAttribute('href', 'https://decentraland.zone/blog')

    fireEvent.click(cards[0])
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Blog', title: 'A Post' })
  })
})
