import { describe, expect, it } from 'vitest'

import { isImageUrl, oldestFirst, relativeDate } from './hub-format'

const NOW = Date.parse('2026-09-21T12:00:00Z')

describe('when a date is shown relative to today', () => {
  it('should read older than a month as months or years, and newer as the date', () => {
    expect(relativeDate('2026-08-21T12:00:00Z', NOW)).toBe('1 month ago')
    expect(relativeDate('2026-04-11T20:05:00Z', NOW)).toBe('5 months ago')
    expect(relativeDate('2025-09-01T00:00:00Z', NOW)).toBe('1 year ago')
    expect(relativeDate('2024-06-01T10:00:00Z', NOW)).toBe('2 years ago')
    expect(relativeDate('2026-09-18T17:40:00Z', NOW)).not.toMatch(/ago/)
  })

  it('should show nothing for a missing or invalid date', () => {
    expect(relativeDate(null, NOW)).toBe('')
    expect(relativeDate('not a date', NOW)).toBe('')
  })
})

describe('when announcements are ordered', () => {
  it('should put the oldest first and an undated one before the dated ones', () => {
    const posts = [
      { createdAt: '2026-09-18T00:00:00Z' },
      { createdAt: null },
      { createdAt: '2024-01-01T00:00:00Z' }
    ]
    expect(oldestFirst(posts).map((post) => post.createdAt)).toEqual([
      null,
      '2024-01-01T00:00:00Z',
      '2026-09-18T00:00:00Z'
    ])
  })
})

describe('when an image address is checked', () => {
  it('should load http(s) addresses and paths of this site, and refuse a bare content hash', () => {
    expect(isImageUrl('https://example.org/a.png')).toBe(true)
    expect(isImageUrl('/assets-cdn/social/communities/c/raw-thumbnail.png')).toBe(true)
    expect(isImageUrl('bafkreibzxeehnngvit3suddejpmbe746vn7xabida6tqptltbcoao32j3u')).toBe(false)
    expect(isImageUrl(null)).toBe(false)
    expect(isImageUrl('')).toBe(false)
  })
})
