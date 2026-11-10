import { describe, expect, it } from 'vitest'
import {
  blogBadgeTone,
  blogCategoryHref,
  blogInitials,
  blogPostHref,
  blogShareLinks,
  formatBlogDate,
  newestBlogCard
} from './blogModel'

describe('formatBlogDate', () => {
  it('formats an ISO date as "Mon DD, YYYY"', () => {
    expect(formatBlogDate('2026-07-02')).toBe('Jul 02, 2026')
  })

  it('returns an empty string for an empty, malformed or out-of-range date', () => {
    expect(formatBlogDate('')).toBe('')
    expect(formatBlogDate('JUL 2, 2026')).toBe('')
    expect(formatBlogDate('2026-13-01')).toBe('')
    expect(formatBlogDate('2026-00-01')).toBe('')
  })
})

describe('newestBlogCard', () => {
  it('picks the latest date, whatever the list order', () => {
    const cards = [
      { publishedAt: '2026-06-29' },
      { publishedAt: '2026-07-02' },
      { publishedAt: '2026-07-01' }
    ]
    expect(newestBlogCard(cards)).toBe(cards[1])
  })

  it('prefers a dated card over an undated one and returns undefined for no cards', () => {
    const cards = [{ publishedAt: '' }, { publishedAt: '2020-01-01' }]
    expect(newestBlogCard(cards)).toBe(cards[1])
    expect(newestBlogCard([{ publishedAt: '' }])).toEqual({ publishedAt: '' })
    expect(newestBlogCard([])).toBeUndefined()
  })
})

describe('links', () => {
  it('encodes slugs and categories', () => {
    expect(blogPostHref('a b')).toBe('/blog/a%20b')
    expect(blogCategoryHref('')).toBe('/blog')
    expect(blogCategoryHref('platform')).toBe('/blog?category=platform')
  })

  it('builds encoded X and LinkedIn share URLs', () => {
    const links = blogShareLinks('A & B', 'https://example.org/blog/a?b=1')
    expect(links.x).toBe(
      'https://x.com/intent/post?text=A%20%26%20B&url=https%3A%2F%2Fexample.org%2Fblog%2Fa%3Fb%3D1'
    )
    expect(links.linkedin).toBe(
      'https://www.linkedin.com/sharing/share-offsite/?url=https%3A%2F%2Fexample.org%2Fblog%2Fa%3Fb%3D1'
    )
  })
})

describe('badges and initials', () => {
  it('maps known categories to tones and falls back to ruby', () => {
    expect(blogBadgeTone('platform')).toBe('violet')
    expect(blogBadgeTone('announcements')).toBe('ruby')
    expect(blogBadgeTone('unknown')).toBe('ruby')
  })

  it('takes up to two initials and handles blank names', () => {
    expect(blogInitials('jane  doe smith')).toBe('JD')
    expect(blogInitials('Catalyrst')).toBe('C')
    expect(blogInitials('   ')).toBe('')
  })
})
