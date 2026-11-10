import { useState } from 'react'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Carousel } from './Carousel'

const labels = {
  previous: 'Previous slide',
  next: 'Next slide',
  goTo: (slide: number) => `Go to slide ${slide}`,
  position: (slide: number, total: number) => `Slide ${slide} of ${total}`
}

/** A slide with its own state, to tell a kept element from a remounted one. */
function Counter({ name }: { name: string }) {
  const [count, setCount] = useState(0)
  return (
    <button type="button" onClick={() => setCount(count + 1)}>
      {`${name} ${count}`}
    </button>
  )
}

function renderCarousel(items: string[], slideClassName?: string, autoplayDelay = 0) {
  return render(
    <>
      <h2 id="title">Clips</h2>
      <Carousel
        items={items}
        autoplayDelay={autoplayDelay}
        getKey={(item) => item}
        labelledBy="title"
        labels={labels}
        slideClassName={slideClassName}
        renderSlide={(item, peek) =>
          peek ? <span data-peek={item}>{item}</span> : <Counter key={item} name={item} />
        }
      />
    </>
  )
}

describe('Carousel', () => {
  describe('when it has several items', () => {
    it('should render the neighbours as inert peeks, hidden from assistive technology', () => {
      const { container } = renderCarousel(['a', 'b', 'c'], 'custom')

      const peeks = [...container.querySelectorAll('[data-peek]')].map(
        (peek) => peek.parentElement!
      )
      expect(peeks.map((peek) => peek.textContent)).toEqual(['c', 'b'])
      for (const peek of peeks) {
        expect(peek).toHaveAttribute('aria-hidden', 'true')
        expect(peek).toHaveAttribute('inert')
      }
      expect(screen.getByRole('group', { name: 'Clips' })).toHaveAttribute(
        'aria-roledescription',
        'carousel'
      )
    })

    it('should give a keyed slide fresh state when the current item changes', async () => {
      renderCarousel(['a', 'b'])

      await userEvent.click(screen.getByRole('button', { name: 'a 0' }))
      expect(screen.getByRole('button', { name: 'a 1' })).toBeInTheDocument()
      await userEvent.click(screen.getByRole('button', { name: 'Next slide' }))
      expect(screen.getByRole('button', { name: 'b 0' })).toBeInTheDocument()
      await userEvent.click(screen.getByRole('button', { name: 'Previous slide' }))
      expect(screen.getByRole('button', { name: 'a 0' })).toBeInTheDocument()
    })
  })

  describe('when the keyboard moves it from inside the current slide', () => {
    it('should move focus to the new slide when its focused content is replaced, and keep moving', async () => {
      const user = userEvent.setup()
      renderCarousel(['a', 'b', 'c'])

      screen.getByRole('button', { name: 'a 0' }).focus()
      await user.keyboard('{ArrowRight}')
      expect(screen.getByRole('button', { name: 'b 0' })).toHaveFocus()
      await user.keyboard('{ArrowRight}')
      expect(screen.getByRole('button', { name: 'c 0' })).toHaveFocus()
      await user.keyboard('{ArrowLeft}{ArrowLeft}')
      expect(screen.getByRole('button', { name: 'a 0' })).toHaveFocus()
    })

    it('should focus the slide itself when the new one has no control', async () => {
      const user = userEvent.setup()
      render(
        <>
          <h2 id="title">Clips</h2>
          <Carousel
            items={['a', 'b']}
            getKey={(item) => item}
            labelledBy="title"
            labels={labels}
            renderSlide={(item, peek) =>
              peek || item === 'b' ? (
                <span key={item}>{item}</span>
              ) : (
                <Counter key={item} name={item} />
              )
            }
          />
        </>
      )

      screen.getByRole('button', { name: 'a 0' }).focus()
      await user.keyboard('{ArrowRight}')
      expect(screen.getByRole('group', { name: 'Slide 2 of 2' })).toHaveFocus()
      // The slide keeps focus as it moves on, and Tab reaches the new slide's control.
      await user.keyboard('{ArrowRight}')
      expect(screen.getByRole('group', { name: 'Slide 1 of 2' })).toHaveFocus()
      await user.tab()
      expect(screen.getByRole('button', { name: 'a 0' })).toHaveFocus()
    })

    it('should leave focus alone when it is outside the current slide', async () => {
      const user = userEvent.setup()
      renderCarousel(['a', 'b'])

      screen.getByRole('button', { name: 'Go to slide 1' }).focus()
      await user.keyboard('{ArrowRight}')
      expect(screen.getByRole('button', { name: 'Go to slide 1' })).toHaveFocus()
    })
  })

  describe('when it has one item', () => {
    it('should show it alone, without peeks, arrows or dots', () => {
      const { container } = renderCarousel(['a'])

      expect(screen.getByRole('group', { name: 'Slide 1 of 1' })).toHaveTextContent('a 0')
      expect(container.querySelector('[data-peek]')).toBeNull()
      expect(screen.queryByRole('button', { name: 'Next slide' })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Go to slide 1' })).not.toBeInTheDocument()
    })
  })

  describe('when it has no items', () => {
    it('should render nothing', () => {
      renderCarousel([])

      expect(screen.queryByRole('group')).not.toBeInTheDocument()
    })
  })
})

describe('when a carousel opts into source autoplay', () => {
  let reduced: boolean
  let listeners: Set<() => void>
  beforeEach(() => {
    vi.useFakeTimers()
    reduced = false
    listeners = new Set()
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        get matches() {
          return reduced
        },
        addEventListener: (_event: string, listener: () => void) => listeners.add(listener),
        removeEventListener: (_event: string, listener: () => void) => listeners.delete(listener)
      }))
    )
  })
  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })
  it('should pause while hovered and restart its full interval after leaving', () => {
    renderCarousel(['a', 'b', 'c'], undefined, 4000)
    fireEvent.pointerOver(screen.getByRole('group', { name: 'Clips' }), { pointerType: 'mouse' })
    act(() => vi.advanceTimersByTime(8000))
    expect(screen.getByRole('button', { name: 'a 0' })).toBeInTheDocument()
    fireEvent.pointerOut(screen.getByRole('group', { name: 'Clips' }), {
      relatedTarget: document.body,
      pointerType: 'mouse'
    })
    act(() => vi.advanceTimersByTime(4000))
    expect(screen.getByRole('button', { name: 'b 0' })).toBeInTheDocument()
  })
  it('should not move a visitor away from a focused slide control', () => {
    renderCarousel(['a', 'b'], undefined, 4000)
    act(() => screen.getByRole('button', { name: 'a 0' }).focus())
    act(() => vi.advanceTimersByTime(12000))
    expect(screen.getByRole('button', { name: 'a 0' })).toHaveFocus()
  })
  it('should stop autoplay immediately when reduced motion is enabled', () => {
    renderCarousel(['a', 'b'], undefined, 4000)
    act(() => {
      reduced = true
      for (const listener of listeners) listener()
    })
    act(() => vi.advanceTimersByTime(12000))
    expect(screen.getByRole('button', { name: 'a 0' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Next slide' }))
    expect(screen.getByRole('group', { name: 'Slide 2 of 2' }).parentElement).toHaveAttribute(
      'data-animated',
      'false'
    )
  })
  it('should settle an infinite wrap without leaving focus in an inert clone', () => {
    renderCarousel(['a', 'b', 'c'], undefined, 4000)
    act(() => screen.getByRole('button', { name: 'a 0' }).focus())
    fireEvent.keyDown(screen.getByRole('button', { name: 'a 0' }), { key: 'ArrowLeft' })
    expect(screen.getByRole('button', { name: 'c 0' })).toHaveFocus()
    fireEvent.transitionEnd(screen.getByRole('group', { name: 'Slide 3 of 3' }).parentElement!)
    expect(screen.getByRole('button', { name: 'c 0' })).toHaveFocus()
    expect(document.activeElement!.closest('[inert]')).toBeNull()
  })
  it('should pause in a background tab and resume only when visible', () => {
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(false)
    renderCarousel(['a', 'b'], undefined, 4000)
    hidden.mockReturnValue(true)
    act(() => document.dispatchEvent(new Event('visibilitychange')))
    act(() => vi.advanceTimersByTime(8000))
    expect(screen.getByRole('button', { name: 'a 0' })).toBeInTheDocument()
    hidden.mockReturnValue(false)
    act(() => document.dispatchEvent(new Event('visibilitychange')))
    act(() => vi.advanceTimersByTime(4000))
    expect(screen.getByRole('button', { name: 'b 0' })).toBeInTheDocument()
  })
  it('should release autoplay and transition timers when unmounted', () => {
    const { unmount } = renderCarousel(['a', 'b'], undefined, 4000)
    expect(vi.getTimerCount()).toBeGreaterThan(0)
    unmount()
    expect(vi.getTimerCount()).toBe(0)
  })
  it.each([0, -1, Number.NaN, Number.POSITIVE_INFINITY])(
    'should keep invalid or disabled interval %s manual',
    (delay) => {
      renderCarousel(['a', 'b'], undefined, delay)
      act(() => vi.advanceTimersByTime(12000))
      expect(screen.getByRole('button', { name: 'a 0' })).toBeInTheDocument()
    }
  )
})
