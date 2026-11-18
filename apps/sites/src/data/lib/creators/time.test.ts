import { describe, it, expect } from 'vitest'
import { formatLongDate, formatTimeAgo } from './time'

const NOW = +new Date('2026-08-20T12:00:00Z')

describe('formatTimeAgo', () => {
  it('picks the largest whole unit', () => {
    expect(formatTimeAgo(NOW - 21 * 24 * 3_600_000, 'en', NOW)).toBe('3 weeks ago')
    expect(formatTimeAgo(NOW - 2 * 24 * 3_600_000, 'en', NOW)).toBe('2 days ago')
    expect(formatTimeAgo(NOW - 5 * 3_600_000, 'en', NOW)).toBe('5 hours ago')
    expect(formatTimeAgo(NOW - 400 * 24 * 3_600_000, 'en', NOW)).toBe('1 year ago')
  })

  it('clamps anything under a minute to 1 minute ago', () => {
    expect(formatTimeAgo(NOW - 10_000, 'en', NOW)).toBe('1 minute ago')
    expect(formatTimeAgo(NOW, 'en', NOW)).toBe('1 minute ago')
  })

  it('localizes', () => {
    expect(formatTimeAgo(NOW - 21 * 24 * 3_600_000, 'es', NOW)).toBe('hace 3 semanas')
  })
})

describe('formatLongDate', () => {
  it('spells the date out in the given locale', () => {
    expect(formatLongDate('2026-02-11T12:00:00Z', 'en')).toBe('February 11, 2026')
    expect(formatLongDate('2026-02-11T12:00:00Z', 'es')).toBe('11 de febrero de 2026')
  })

  it('keeps a date-only value on its calendar day whatever the local zone', () => {
    expect(formatLongDate('2026-02-11', 'en')).toBe('February 11, 2026')
  })

  it('is empty for a missing or unparsable date', () => {
    expect(formatLongDate(null, 'en')).toBe('')
    expect(formatLongDate('not a date', 'en')).toBe('')
  })
})
