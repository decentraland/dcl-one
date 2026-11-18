import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { Carousel } from './Carousel'

const items = [
  { id: 'a', text: 'Slide A' },
  { id: 'b', text: 'Slide B' },
  { id: 'c', text: 'Slide C' }
]

function renderCarousel(autoplayMs = 0) {
  return render(
    <TranslationProvider>
      <Carousel
        items={items}
        renderItem={item => (
          <a href={`/${item.id}`} data-testid="card">
            {item.text}
          </a>
        )}
        keyExtractor={item => item.id}
        label="Testimonials"
        slideWidth={500}
        autoplayMs={autoplayMs}
      />
    </TranslationProvider>
  )
}

const activeSlide = () => screen.getAllByTestId('carousel-slide').find(slide => slide.hasAttribute('data-active'))
const currentDot = () => screen.getByTestId('carousel-dots').querySelector('[aria-current="true"]')
const track = () => screen.getByTestId('carousel-track')

// The browser ends the slide animation; jsdom does not, so the test signals it.
const settle = () => fireEvent.transitionEnd(track())

const next = () => {
  fireEvent.click(screen.getByRole('button', { name: 'Next slide' }))
  settle()
}
const prev = () => {
  fireEvent.click(screen.getByRole('button', { name: 'Previous slide' }))
  settle()
}

describe('Carousel', () => {
  it('starts on the first slide, with only that slide exposed to assistive tech', () => {
    renderCarousel()
    expect(screen.getByRole('region', { name: 'Testimonials' })).toBeInTheDocument()
    expect(activeSlide()).toHaveTextContent('Slide A')
    expect(screen.getAllByRole('link')).toHaveLength(1)
    expect(currentDot()).toHaveAccessibleName('Go to slide 1')
  })

  it('keeps the links of every other slide, clones included, out of keyboard reach', () => {
    renderCarousel()
    const slides = screen.getAllByTestId('carousel-slide')
    expect(slides).toHaveLength(9)
    for (const slide of slides) {
      if (slide === activeSlide()) expect(slide).not.toHaveAttribute('inert')
      else expect(slide).toHaveAttribute('inert')
    }
    next()
    expect(activeSlide()).toHaveTextContent('Slide B')
    expect(activeSlide()).not.toHaveAttribute('inert')
    expect(slides.filter(slide => !slide.hasAttribute('inert'))).toHaveLength(1)
  })

  it('loops endlessly: next on the last slide shows the first, previous on the first shows the last', () => {
    renderCarousel()
    next()
    expect(activeSlide()).toHaveTextContent('Slide B')
    next()
    expect(activeSlide()).toHaveTextContent('Slide C')
    next()
    expect(activeSlide()).toHaveTextContent('Slide A')
    expect(currentDot()).toHaveAccessibleName('Go to slide 1')

    prev()
    expect(activeSlide()).toHaveTextContent('Slide C')
    expect(currentDot()).toHaveAccessibleName('Go to slide 3')
    prev()
    prev()
    prev()
    expect(activeSlide()).toHaveTextContent('Slide C')
  })

  it('jumps straight to a slide from its dot', () => {
    renderCarousel()
    fireEvent.click(screen.getByRole('button', { name: 'Go to slide 3' }))
    settle()
    expect(activeSlide()).toHaveTextContent('Slide C')
    expect(currentDot()).toHaveAccessibleName('Go to slide 3')
  })

  it('swipes to the next and previous slide, ignoring short drags', () => {
    renderCarousel()
    fireEvent.touchStart(track(), { touches: [{ clientX: 300 }] })
    fireEvent.touchMove(track(), { touches: [{ clientX: 200 }] })
    fireEvent.touchEnd(track())
    settle()
    expect(activeSlide()).toHaveTextContent('Slide B')

    fireEvent.touchStart(track(), { touches: [{ clientX: 300 }] })
    fireEvent.touchMove(track(), { touches: [{ clientX: 280 }] })
    fireEvent.touchEnd(track())
    settle()
    expect(activeSlide()).toHaveTextContent('Slide B')

    fireEvent.mouseDown(track(), { button: 0, clientX: 100 })
    fireEvent.mouseMove(window, { clientX: 300 })
    fireEvent.mouseUp(window)
    settle()
    expect(activeSlide()).toHaveTextContent('Slide A')
  })

  it('does not open the card a mouse drag ends on', () => {
    renderCarousel()
    const onClick = vi.fn((event: Event) => event.preventDefault())
    screen.getAllByTestId('card').forEach(card => card.addEventListener('click', onClick))

    fireEvent.mouseDown(track(), { button: 0, clientX: 100 })
    fireEvent.mouseMove(window, { clientX: 120 })
    fireEvent.mouseUp(window)
    fireEvent.click(activeSlide()!.querySelector('a')!)
    expect(onClick).not.toHaveBeenCalled()

    fireEvent.click(activeSlide()!.querySelector('a')!)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  describe('autoplay', () => {
    beforeEach(() => vi.useFakeTimers())
    afterEach(() => vi.useRealTimers())

    it('advances on its own, waits while the tab is hidden and resumes when it is shown again', () => {
      renderCarousel(1000)
      act(() => {
        vi.advanceTimersByTime(1000)
      })
      settle()
      expect(activeSlide()).toHaveTextContent('Slide B')

      Object.defineProperty(document, 'hidden', { configurable: true, value: true })
      fireEvent(document, new Event('visibilitychange'))
      act(() => {
        vi.advanceTimersByTime(3000)
      })
      expect(activeSlide()).toHaveTextContent('Slide B')

      Object.defineProperty(document, 'hidden', { configurable: true, value: false })
      fireEvent(document, new Event('visibilitychange'))
      act(() => {
        vi.advanceTimersByTime(1000)
      })
      settle()
      expect(activeSlide()).toHaveTextContent('Slide C')
    })
  })
})
