import { describe, expect, it } from 'vitest'
import {
  blogAuthorHref,
  blogBadgeTone,
  blogCategoryHref,
  blogInitials,
  blogPostHref,
  blogShareLinks,
  formatBlogDate
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

describe('links', () => {
  it('builds decentraland/sites-shaped paths and encodes each segment', () => {
    expect(blogPostHref('platform', 'a b')).toBe('/blog/platform/a%20b')
    expect(blogPostHref('x/y', 'p?q')).toBe('/blog/x%2Fy/p%3Fq')
    expect(blogCategoryHref('')).toBe('/blog')
    expect(blogCategoryHref('platform')).toBe('/blog/platform')
    expect(blogAuthorHref('jane doe')).toBe('/blog/author/jane%20doe')
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
