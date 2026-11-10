import { fireEvent, render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { formatBlogDate, inAppLink, shareLinks } from './blogLinks'

describe('formatBlogDate', () => {
  it.each([
    ['2023-04-06T00:00+02:00', 'Apr 05, 2023'],
    ['2020-08-25T12:00:00.000Z', 'Aug 25, 2020'],
    // A date-only value is UTC midnight.
    ['2020-08-25', 'Aug 25, 2020']
  ])('formats %s as source does (en-US, UTC): %s', (value, expected) => {
    expect(formatBlogDate(value)).toBe(expected)
  })

  it('gives an empty string for no value, and an unparseable value as given', () => {
    expect(formatBlogDate('')).toBe('')
    expect(formatBlogDate(undefined)).toBe('')
    expect(formatBlogDate('not a date')).toBe('not a date')
  })
})

describe('shareLinks', () => {
  it('builds the source X intent and Facebook sharer URLs, each value encoded', () => {
    expect(shareLinks('A & B', 'https://example.org/blog/a/b?x=1')).toEqual({
      x: 'https://x.com/intent/post?text=A%20%26%20B&url=https%3A%2F%2Fexample.org%2Fblog%2Fa%2Fb%3Fx%3D1',
      facebook:
        'https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Fexample.org%2Fblog%2Fa%2Fb%3Fx%3D1'
    })
  })
})

describe('inAppLink', () => {
  const link = (onNavigate?: (href: string) => void) =>
    render(
      <a href="/blog/x" {...inAppLink('/blog/x', onNavigate)}>
        x
      </a>
    ).container.querySelector('a')!

  it('follows a plain primary click in the app', () => {
    const onNavigate = vi.fn()
    const event = fireEvent.click(link(onNavigate))
    expect(onNavigate).toHaveBeenCalledWith('/blog/x')
    expect(event).toBe(false) // default prevented
  })

  it.each([
    { metaKey: true },
    { ctrlKey: true },
    { shiftKey: true },
    { altKey: true },
    { button: 1 }
  ])('leaves a modified or non-primary click to the browser (%o)', (init) => {
    const onNavigate = vi.fn()
    expect(fireEvent.click(link(onNavigate), init)).toBe(true)
    expect(onNavigate).not.toHaveBeenCalled()
  })

  it('adds nothing without a navigate function', () => {
    expect(inAppLink('/blog/x', undefined)).toEqual({})
  })
})
