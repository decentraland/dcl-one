import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Rail } from '../design/Rail'

// jsdom has no element scrolling; a test that follows the scroll replaces this with its own.
const scrollTo = HTMLElement.prototype.scrollTo
beforeEach(() => {
  HTMLElement.prototype.scrollTo = function (): void {}
})
afterEach(() => {
  HTMLElement.prototype.scrollTo = scrollTo
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('Rail pagination', () => {
  it('keeps Next available for the last card of seven and supports returning from the clamped final page', () => {
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(280)
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(864)
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(2008)
    const original = HTMLElement.prototype.scrollTo
    HTMLElement.prototype.scrollTo = function (options?: ScrollToOptions | number): void {
      this.scrollLeft = Math.min(typeof options === 'object' ? options.left ?? 0 : options ?? 0, 1144)
      fireEvent.scroll(this)
    }
    try {
      render(<Rail perPage={3} gap={8}>{Array.from({ length: 7 }, (_, i) => <div key={i}>Place {i + 1}</div>)}</Rail>)
      fireEvent.click(screen.getByRole('button', { name: 'Next' }))
      expect(screen.getByRole('button', { name: 'Page 2' })).toHaveAttribute('aria-current', 'true')
      fireEvent.click(screen.getByRole('button', { name: 'Next' }))
      expect(screen.getByRole('button', { name: 'Page 3' })).toHaveAttribute('aria-current', 'true')
      expect(screen.queryByRole('button', { name: 'Next' })).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'Previous' }))
      expect(screen.getByRole('button', { name: 'Page 2' })).toHaveAttribute('aria-current', 'true')
      fireEvent.click(screen.getByRole('button', { name: 'Previous' }))
      expect(screen.getByRole('button', { name: 'Page 1' })).toHaveAttribute('aria-current', 'true')
    } finally {
      HTMLElement.prototype.scrollTo = original
    }
  })

  // Five 295px cards 20px apart (1555px), as the Shop's outfits, with the home's 12px bleed on each side.
  const fiveOutfits = (): React.JSX.Element => (
    <Rail perPage={4} gap={20} bleed={12}>
      {Array.from({ length: 5 }, (_, i) => (
        <div key={i}>Outfit {i + 1}</div>
      ))}
    </Rail>
  )
  const layout = (clientWidth: number): void => {
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(295)
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() => clientWidth)
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(12 + 1555 + 12)
  }
  const paged = (): boolean => screen.queryByRole('button', { name: 'Next' }) != null || screen.queryByRole('button', { name: 'Page 1' }) != null

  it('shows neither arrows nor dots when every card fits', () => {
    layout(1700)
    render(fiveOutfits())
    expect(paged()).toBe(false)
  })

  it('doesn’t count the end bleed as another page', () => {
    // The cards and the start bleed fit; only the end bleed's 12px is past the edge.
    layout(12 + 1555)
    render(fiveOutfits())
    expect(paged()).toBe(false)
  })

  it('pages the cards that don’t fit, and drops the pages a wider viewport no longer needs', () => {
    const resized: Array<() => void> = []
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(callback: () => void) {
          resized.push(callback)
        }
        observe(): void {}
        disconnect(): void {}
      }
    )
    let width = 1300
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(295)
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() => width)
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(12 + 1555 + 12)
    render(fiveOutfits())
    expect(screen.getByRole('button', { name: 'Next' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Page 2' })).toBeInTheDocument()
    width = 1700
    act(() => resized.forEach((callback) => callback()))
    expect(paged()).toBe(false)
  })
})
