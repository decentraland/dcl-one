import { readFileSync } from 'node:fs'
import path from 'node:path'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { WeeklyRituals } from './WeeklyRituals'

const card = (id: string, title: string) => ({
  id,
  title,
  href: `/whats-on/${id}`,
  image: `/${id}.webp`,
  mobileImage: `/${id}-mobile.webp`
})
const cards = [
  card('career-mondays', 'Career Mondays'),
  card('takeover-tuesdays', 'Takeover Tuesdays'),
  card('watch-party', 'Watch Party')
]
const labels = {
  previous: 'Previous slide',
  next: 'Next slide',
  goTo: (slide: number) => `Go to slide ${slide}`,
  position: (slide: number, total: number) => `Slide ${slide} of ${total}`
}

function renderRituals(onCardClick = vi.fn()) {
  render(
    <WeeklyRituals
      title="Your Weekly Rituals"
      cards={cards}
      labels={labels}
      onCardClick={onCardClick}
    />
  )
  return { onCardClick }
}

const current = () => screen.getByRole('group', { name: /^Slide \d of 3$/ })

describe('WeeklyRituals', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  describe('when rendered', () => {
    it('should show the first ritual as the only link, named by its artwork, in a labelled carousel', () => {
      renderRituals()

      const carousel = screen.getByRole('group', { name: 'Your Weekly Rituals' })
      expect(carousel).toHaveAttribute('aria-roledescription', 'carousel')
      expect(current()).toHaveAccessibleName('Slide 1 of 3')
      expect(screen.getAllByRole('link')).toHaveLength(1)
      expect(within(current()).getByRole('link', { name: 'Career Mondays' })).toHaveAttribute(
        'href',
        '/whats-on/career-mondays'
      )
    })

    it('should show the neighbours as decorative, hidden peeks', () => {
      const { container } = render(<WeeklyRituals title="t" cards={cards} labels={labels} />)

      const peeks = container.querySelectorAll('[aria-hidden="true"] img')
      expect([...peeks].map((img) => img.getAttribute('src'))).toEqual([
        '/watch-party.webp',
        '/takeover-tuesdays.webp'
      ])
      expect([...peeks].every((img) => img.getAttribute('alt') === '')).toBe(true)
    })

    it('should offer the portrait artwork to narrow screens', () => {
      renderRituals()

      const source = current().querySelector('picture source')
      expect(source).toHaveAttribute('media', '(max-width: 767px)')
      expect(source).toHaveAttribute('srcset', '/career-mondays-mobile.webp')
    })
  })

  describe('when moved', () => {
    it('should go forward and back with the arrows, wrapping at the ends', async () => {
      renderRituals()

      await userEvent.click(screen.getByRole('button', { name: 'Previous slide' }))
      expect(current()).toHaveAccessibleName('Slide 3 of 3')
      await userEvent.click(screen.getByRole('button', { name: 'Next slide' }))
      await userEvent.click(screen.getByRole('button', { name: 'Next slide' }))
      expect(current()).toHaveAccessibleName('Slide 2 of 3')
      expect(screen.getByRole('link', { name: 'Takeover Tuesdays' })).toBeInTheDocument()
    })

    it('should jump with the dots, marking the current one', async () => {
      renderRituals()

      await userEvent.click(screen.getByRole('button', { name: 'Go to slide 3' }))
      expect(current()).toHaveAccessibleName('Slide 3 of 3')
      expect(screen.getByRole('button', { name: 'Go to slide 3' })).toHaveAttribute(
        'aria-current',
        'true'
      )
      expect(screen.getByRole('button', { name: 'Go to slide 1' })).not.toHaveAttribute(
        'aria-current'
      )
    })

    it('should move with the Left and Right keys from inside the carousel', async () => {
      const user = userEvent.setup()
      renderRituals()

      screen.getByRole('link', { name: 'Career Mondays' }).focus()
      await user.keyboard('{ArrowRight}')
      expect(current()).toHaveAccessibleName('Slide 2 of 3')
      // The same link keeps focus while the slide changes.
      expect(screen.getByRole('link', { name: 'Takeover Tuesdays' })).toHaveFocus()
      await user.keyboard('{ArrowLeft}{ArrowLeft}')
      expect(current()).toHaveAccessibleName('Slide 3 of 3')
    })

    it('should keep reduced-motion visitors on their chosen slide without animation', () => {
      vi.useFakeTimers()
      vi.stubGlobal(
        'matchMedia',
        vi.fn(() => ({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() }))
      )
      renderRituals()
      act(() => vi.advanceTimersByTime(60_000))
      expect(current()).toHaveAccessibleName('Slide 1 of 3')
      fireEvent.click(screen.getByRole('button', { name: 'Next slide' }))
      expect(current()).toHaveAccessibleName('Slide 2 of 3')
      expect(current().parentElement).toHaveAttribute('data-animated', 'false')
    })
    it('should advance weekly rituals every four seconds when motion is allowed', () => {
      vi.useFakeTimers()
      vi.stubGlobal(
        'matchMedia',
        vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }))
      )
      renderRituals()
      act(() => vi.advanceTimersByTime(3999))
      expect(current()).toHaveAccessibleName('Slide 1 of 3')
      act(() => vi.advanceTimersByTime(1))
      expect(current()).toHaveAccessibleName('Slide 2 of 3')
      expect(current().parentElement).toHaveAttribute('data-animated', 'true')
    })
  })

  describe('when swiped on a touch screen', () => {
    // jsdom has no PointerEvent; a MouseEvent carries the same coordinates.
    const pointer = (type: 'pointerDown' | 'pointerUp', target: Element, x: number, y = 100) =>
      fireEvent[type](target, { clientX: x, clientY: y, pointerType: 'touch' })
    const track = () => current().parentElement!

    it('should move one slide per horizontal swipe, either way', () => {
      renderRituals()

      pointer('pointerDown', track(), 300)
      pointer('pointerUp', track(), 200)
      expect(current()).toHaveAccessibleName('Slide 2 of 3')
      pointer('pointerDown', track(), 100)
      pointer('pointerUp', track(), 220)
      expect(current()).toHaveAccessibleName('Slide 1 of 3')
    })

    it('should not follow the card link at the end of a swipe', () => {
      const { onCardClick } = renderRituals()
      const link = screen.getByRole('link', { name: 'Career Mondays' })

      pointer('pointerDown', link, 300)
      pointer('pointerUp', link, 150)
      const click = new MouseEvent('click', { bubbles: true, cancelable: true })
      screen.getByRole('link', { name: 'Takeover Tuesdays' }).dispatchEvent(click)

      expect(click.defaultPrevented).toBe(true)
      expect(onCardClick).not.toHaveBeenCalled()
    })

    it('should not swallow a later keyboard activation when the swipe ended off the link', () => {
      vi.useFakeTimers()
      const { onCardClick } = renderRituals()

      pointer('pointerDown', track(), 300)
      pointer('pointerUp', track(), 150)
      // The carousel clears its swipe guard on the next tick; run that tick instead of waiting for it.
      vi.advanceTimersByTime(0)
      const link = screen.getByRole('link', { name: 'Takeover Tuesdays' })
      link.addEventListener('click', (event) => event.preventDefault())
      link.click()

      expect(onCardClick).toHaveBeenCalledWith(expect.objectContaining({ id: 'takeover-tuesdays' }))
    })

    it('should follow the link on a plain tap', () => {
      const { onCardClick } = renderRituals()
      const link = screen.getByRole('link', { name: 'Career Mondays' })
      link.addEventListener('click', (event) => event.preventDefault())

      pointer('pointerDown', link, 300)
      pointer('pointerUp', link, 304)
      link.click()

      expect(current()).toHaveAccessibleName('Slide 1 of 3')
      expect(onCardClick).toHaveBeenCalledWith(expect.objectContaining({ id: 'career-mondays' }))
    })

    it('should capture a mouse drag on the whole carousel so an outside release still ends it', () => {
      renderRituals()
      const setPointerCapture = vi.fn()
      const carousel = screen.getByRole('group', { name: 'Your Weekly Rituals' })
      carousel.setPointerCapture = setPointerCapture

      fireEvent.pointerDown(track(), {
        clientX: 300,
        clientY: 100,
        pointerType: 'mouse',
        pointerId: 7
      })
      // Not on press: a plain click must still reach the card link.
      expect(setPointerCapture).not.toHaveBeenCalled()
      fireEvent.pointerMove(track(), {
        clientX: 220,
        clientY: 102,
        pointerType: 'mouse',
        pointerId: 7
      })
      expect(setPointerCapture).toHaveBeenCalledWith(7)
      // Capture delivers the release to the owning hit area wherever it happens.
      fireEvent.pointerUp(carousel, {
        clientX: -40,
        clientY: 110,
        pointerType: 'mouse',
        pointerId: 7
      })
      expect(current()).toHaveAccessibleName('Slide 2 of 3')
    })

    it('should leave a mostly vertical gesture to page scrolling', () => {
      renderRituals()

      pointer('pointerDown', track(), 300, 100)
      pointer('pointerUp', track(), 240, 400)
      expect(current()).toHaveAccessibleName('Slide 1 of 3')
      const css = readFileSync(
        path.resolve(import.meta.dirname, '../shared/Carousel.module.css'),
        'utf8'
      )
      expect(css).toMatch(/\.carousel \{[^}]*touch-action: pan-y;/)
    })
  })

  describe('when a ritual is followed', () => {
    it('should report which card, for analytics', async () => {
      const { onCardClick } = renderRituals()

      await userEvent.click(screen.getByRole('button', { name: 'Next slide' }))
      screen.getByRole('link', { name: 'Takeover Tuesdays' }).click()
      expect(onCardClick).toHaveBeenCalledWith(expect.objectContaining({ id: 'takeover-tuesdays' }))
    })
  })
})
