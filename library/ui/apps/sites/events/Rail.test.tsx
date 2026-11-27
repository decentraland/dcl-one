import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Rail } from './Rail'

const nativeScrollTo = HTMLElement.prototype.scrollTo

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  HTMLElement.prototype.scrollTo = nativeScrollTo
})

describe('when a rail is three viewports wide', () => {
  let scrollTo: ReturnType<typeof vi.fn>
  let scrollLeft: number

  beforeEach(() => {
    scrollLeft = 0
    scrollTo = vi.fn()
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(300)
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(900)
    vi.spyOn(HTMLElement.prototype, 'scrollLeft', 'get').mockImplementation(() => scrollLeft)
    HTMLElement.prototype.scrollTo = scrollTo as unknown as HTMLElement['scrollTo']
    render(
      <Rail label="live scenes" count={3}>
        <div>One</div>
      </Rail>
    )
  })

  /** The browser scrolled the track to a page; the rail reads it on scroll. */
  function scrollToPage(page: number) {
    scrollLeft = page * 300
    fireEvent.scroll(screen.getByText('One').parentElement!)
  }

  it('should offer only the next page on the first one, as a dark icon-only button', async () => {
    expect(screen.queryByRole('button', { name: 'Show previous live scenes' })).toBeNull()
    const next = screen.getByRole('button', { name: 'Show next live scenes' })
    expect(next.getAttribute('data-variant')).toBe('dark')
    expect(next.getAttribute('data-icon-only')).toBe('true')
    await userEvent.click(next)
    expect(scrollTo).toHaveBeenCalledWith(expect.objectContaining({ left: 300 }))
  })

  it('should offer both directions in the middle and only the previous page on the last one', async () => {
    scrollToPage(1)
    await userEvent.click(screen.getByRole('button', { name: 'Show previous live scenes' }))
    expect(scrollTo).toHaveBeenLastCalledWith(expect.objectContaining({ left: 0 }))
    expect(screen.getByRole('button', { name: 'Show next live scenes' })).toBeTruthy()
    scrollToPage(2)
    expect(screen.queryByRole('button', { name: 'Show next live scenes' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Show previous live scenes' })).toBeTruthy()
  })

  it('should mark the page in view among the page dots', () => {
    scrollToPage(1)
    const current = screen.getByRole('button', { name: 'Page 2 of 3' })
    expect(current.getAttribute('aria-current')).toBe('true')
    expect(
      screen.getByRole('button', { name: 'Page 1 of 3' }).getAttribute('aria-current')
    ).toBeNull()
  })
})

describe('when a rail fits in one viewport', () => {
  it('should show no paging controls at all', () => {
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(300)
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(300)
    render(
      <Rail label="live scenes" count={1}>
        <div>One</div>
      </Rail>
    )
    expect(screen.queryByRole('button')).toBeNull()
  })
})
