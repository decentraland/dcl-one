import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PagedRail } from './PagedRail'

// Renders and lets the measuring effects settle inside act(), so React reports no stray update.
async function renderSettled(ui: ReactElement) {
  let view!: ReturnType<typeof render>
  await act(async () => {
    view = render(ui)
  })
  return view
}

const cards = (count: number) =>
  Array.from({ length: count }, (_, index) => <div key={index}>Card {index + 1}</div>)

/** A layout jsdom does not compute: every card `card` wide in a viewport `viewport` wide over `scroll` of content. */
function layout(card: number, viewport: number | (() => number), scroll: number) {
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(card)
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() =>
    typeof viewport === 'number' ? viewport : viewport()
  )
  vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(scroll)
}

/** Element scrolling, which jsdom lacks: records each call, clamps to `end` and reports the scroll. */
function scrolling(end: number) {
  const calls: ScrollToOptions[] = []
  HTMLElement.prototype.scrollTo = function (options?: ScrollToOptions | number): void {
    const asked = typeof options === 'object' ? options : { left: options ?? 0 }
    calls.push(asked)
    this.scrollLeft = Math.min(asked.left ?? 0, end)
    this.dispatchEvent(new Event('scroll'))
  }
  return calls
}

const button = (name: string) => screen.queryByRole('button', { name })
const current = () =>
  screen
    .getAllByRole('button')
    .filter((dot) => dot.getAttribute('aria-current') === 'true')
    .map((dot) => dot.getAttribute('aria-label'))
const paged = () => button('Next') != null || button('Previous') != null || button('Page 1') != null

const scrollTo = HTMLElement.prototype.scrollTo
beforeEach(() => {
  HTMLElement.prototype.scrollTo = function (): void {}
})
afterEach(() => {
  HTMLElement.prototype.scrollTo = scrollTo
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('PagedRail', () => {
  describe('when it has no cards', () => {
    it('should render an empty row with neither arrows nor dots', async () => {
      layout(0, 600, 600)
      const { container } = await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {[]}
        </PagedRail>
      )
      expect(paged()).toBe(false)
      expect(container.firstElementChild).not.toBeNull()
    })
  })

  describe('when every card fits', () => {
    it('should show neither arrows nor dots for one card', async () => {
      layout(194, 600, 600)
      await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(1)}
        </PagedRail>
      )
      expect(screen.getByText('Card 1')).toBeInTheDocument()
      expect(paged()).toBe(false)
    })

    it('should show neither for more cards than a page holds, in a viewport wide enough', async () => {
      layout(295, 1700, 12 + 1555 + 12)
      await renderSettled(
        <PagedRail perPage={4} gap={20} bleed={12}>
          {cards(5)}
        </PagedRail>
      )
      expect(paged()).toBe(false)
    })

    it('should not count the end bleed as another page', async () => {
      // The cards and the start bleed fit; only the end bleed's 12px is past the edge.
      layout(295, 12 + 1555, 12 + 1555 + 12)
      await renderSettled(
        <PagedRail perPage={4} gap={20} bleed={12}>
          {cards(5)}
        </PagedRail>
      )
      expect(paged()).toBe(false)
    })
  })

  describe('when the viewport has no width yet', () => {
    it('should page by the count of cards', async () => {
      layout(0, 0, 0)
      await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(7)}
        </PagedRail>
      )
      expect(button('Page 3')).not.toBeNull()
      expect(button('Page 4')).toBeNull()
    })

    it('should show one page for exactly a page of cards', async () => {
      layout(0, 0, 0)
      await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(3)}
        </PagedRail>
      )
      expect(paged()).toBe(false)
    })
  })

  describe('when a page is said to hold no card', () => {
    it('should count one card a page before the row is laid out, and not fail', async () => {
      layout(0, 0, 0)
      await renderSettled(
        <PagedRail perPage={0} gap={8}>
          {cards(3)}
        </PagedRail>
      )
      expect(button('Page 3')).not.toBeNull()
      expect(button('Page 4')).toBeNull()
    })

    it('should step one card at a time once it is laid out, for less than one or not a number', async () => {
      // Twenty 194px cards 8px apart over 600px: 3432px of travel, a 202px stride for one card.
      for (const perPage of [-2, Number.NaN]) {
        layout(194, 600, 4032)
        const calls = scrolling(3432)
        const view = await renderSettled(
          <PagedRail perPage={perPage} gap={8}>
            {cards(20)}
          </PagedRail>
        )
        expect(button('Page 18')).not.toBeNull()
        expect(button('Page 19')).toBeNull()
        fireEvent.click(button('Next')!)
        expect(calls.at(-1)?.left).toBe(202)
        view.unmount()
        vi.restoreAllMocks()
      }
    })
  })

  describe('when the cards end exactly on a page', () => {
    it('should page six cards of three as two pages', async () => {
      // Six 194px cards 8px apart are 1204px; three and their gaps are 598px of a 598px viewport. The cards
      // travel 606px, one stride of three.
      layout(194, 598, 1204)
      const calls = scrolling(606)
      await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(6)}
        </PagedRail>
      )
      expect(button('Page 2')).not.toBeNull()
      expect(button('Page 3')).toBeNull()
      fireEvent.click(button('Next')!)
      expect(calls.at(-1)?.left).toBe(606)
      expect(current()).toEqual(['Page 2'])
      expect(button('Next')).toBeNull()
    })

    it('should end the last page at the very end, the end bleed being padding', async () => {
      // Eight 295px cards 20px apart with a 12px bleed on each side: 2524px over a 1252px viewport. The
      // cards travel 1260px, one stride of four; the last 12px are the end bleed.
      layout(295, 1252, 2524)
      const calls = scrolling(1272)
      await renderSettled(
        <PagedRail perPage={4} gap={20} bleed={12}>
          {cards(8)}
        </PagedRail>
      )
      expect(button('Page 2')).not.toBeNull()
      expect(button('Page 3')).toBeNull()
      fireEvent.click(button('Next')!)
      expect(calls.at(-1)?.left).toBe(1272)
      expect(current()).toEqual(['Page 2'])
      fireEvent.click(button('Previous')!)
      expect(calls.at(-1)?.left).toBe(0)
      expect(current()).toEqual(['Page 1'])
      // Scrolled by hand to where the last four cards start (a snap point, short of the end bleed).
      const viewport = screen.getByText('Card 1').parentElement!.parentElement!
      viewport.scrollLeft = 1260
      fireEvent.scroll(viewport)
      expect(current()).toEqual(['Page 2'])
    })
  })

  describe('when it has many pages', () => {
    const twenty = () => (
      <PagedRail perPage={3} gap={8}>
        {cards(20)}
      </PagedRail>
    )

    beforeEach(() => {
      // Twenty 194px cards 8px apart are 4032px over 600px: 3432px of travel, 606px a stride.
      layout(194, 600, 4032)
    })

    it('should show a dot per page, the first current, and no previous arrow', async () => {
      scrolling(3432)
      await renderSettled(twenty())
      expect(button('Page 7')).not.toBeNull()
      expect(button('Page 8')).toBeNull()
      expect(current()).toEqual(['Page 1'])
      expect(button('Previous')).toBeNull()
      expect(button('Next')).not.toBeNull()
    })

    it('should step a page at a time, and show both arrows in the middle', async () => {
      const calls = scrolling(3432)
      await renderSettled(twenty())
      fireEvent.click(button('Next')!)
      expect(calls.at(-1)?.left).toBe(606)
      expect(current()).toEqual(['Page 2'])
      expect(button('Previous')).not.toBeNull()
      expect(button('Next')).not.toBeNull()
    })

    it('should go to the page of a pressed dot, and clamp the last one to the end', async () => {
      const calls = scrolling(3432)
      await renderSettled(twenty())
      fireEvent.click(button('Page 4')!)
      expect(calls.at(-1)?.left).toBe(1818)
      expect(current()).toEqual(['Page 4'])
      fireEvent.click(button('Page 7')!)
      expect(calls.at(-1)?.left).toBe(3432)
      expect(current()).toEqual(['Page 7'])
      expect(button('Next')).toBeNull()
      expect(button('Previous')).not.toBeNull()
    })

    it('should return from the clamped last page a page at a time', async () => {
      scrolling(3432)
      await renderSettled(twenty())
      fireEvent.click(button('Page 7')!)
      fireEvent.click(button('Previous')!)
      expect(current()).toEqual(['Page 6'])
    })

    it('should follow a scroll made by hand', async () => {
      await renderSettled(twenty())
      const viewport = screen.getByText('Card 1').parentElement!.parentElement!
      viewport.scrollLeft = 1300
      fireEvent.scroll(viewport)
      expect(current()).toEqual(['Page 3'])
      viewport.scrollLeft = 0
      fireEvent.scroll(viewport)
      expect(current()).toEqual(['Page 1'])
    })

    it('should press its arrows and dots from the keyboard', async () => {
      scrolling(3432)
      await renderSettled(twenty())
      await userEvent.tab()
      expect(button('Next')).toHaveFocus()
      await userEvent.keyboard('{Enter}')
      expect(current()).toEqual(['Page 2'])
      button('Page 5')!.focus()
      await userEvent.keyboard(' ')
      expect(current()).toEqual(['Page 5'])
    })

    it('should move the keyboard focus to the other arrow when the focused one hides at an end, and only then', async () => {
      scrolling(3432)
      await renderSettled(twenty())
      button('Page 7')!.focus()
      await userEvent.keyboard('{Enter}')
      expect(button('Page 7')).toHaveFocus()
      button('Page 6')!.focus()
      await userEvent.keyboard('{Enter}')
      button('Next')!.focus()
      await userEvent.keyboard('{Enter}')
      expect(current()).toEqual(['Page 7'])
      expect(button('Next')).toBeNull()
      expect(button('Previous')).toHaveFocus()
      button('Page 2')!.focus()
      await userEvent.keyboard('{Enter}')
      button('Previous')!.focus()
      await userEvent.keyboard('{Enter}')
      expect(current()).toEqual(['Page 1'])
      expect(button('Previous')).toBeNull()
      expect(button('Next')).toHaveFocus()
    })

    it('should scroll smoothly, and at once for a visitor who asks for less motion', async () => {
      const calls = scrolling(3432)
      await renderSettled(twenty())
      fireEvent.click(button('Next')!)
      expect(calls.at(-1)?.behavior).toBe('smooth')
      vi.stubGlobal('matchMedia', (query: string) => ({
        matches: query === '(prefers-reduced-motion: reduce)'
      }))
      fireEvent.click(button('Next')!)
      expect(calls.at(-1)?.behavior).toBe('auto')
    })

    it('should go back to the first page when the cards change', async () => {
      const calls = scrolling(3432)
      const view = await renderSettled(twenty())
      fireEvent.click(button('Page 4')!)
      await act(async () => {
        view.rerender(
          <PagedRail perPage={3} gap={8}>
            {cards(19)}
          </PagedRail>
        )
      })
      expect(calls.at(-1)?.left).toBe(0)
      expect(current()).toEqual(['Page 1'])
    })
  })

  describe('when the viewport or its cards resize', () => {
    it('should drop the pages a wider viewport no longer needs, and stop observing on unmount', async () => {
      const resized: Array<() => void> = []
      let disconnected = 0
      vi.stubGlobal(
        'ResizeObserver',
        class {
          constructor(callback: () => void) {
            resized.push(callback)
          }
          observe(): void {}
          disconnect(): void {
            disconnected += 1
          }
        }
      )
      let width = 1300
      layout(295, () => width, 12 + 1555 + 12)
      const view = await renderSettled(
        <PagedRail perPage={4} gap={20} bleed={12}>
          {cards(5)}
        </PagedRail>
      )
      expect(button('Next')).not.toBeNull()
      expect(button('Page 2')).not.toBeNull()
      width = 1700
      act(() => resized.forEach((callback) => callback()))
      expect(paged()).toBe(false)
      expect(disconnected).toBe(0)
      view.unmount()
      expect(disconnected).toBe(1)
    })

    it('should keep the current page inside the pages that are left', async () => {
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
      let width = 600
      layout(194, () => width, 4032)
      scrolling(3432)
      await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(20)}
        </PagedRail>
      )
      fireEvent.click(button('Page 7')!)
      width = 3000
      act(() => resized.forEach((callback) => callback()))
      expect(button('Page 3')).not.toBeNull()
      expect(button('Page 4')).toBeNull()
      expect(current()).toEqual(['Page 3'])
    })

    it('should measure on a window resize where there is no ResizeObserver, and stop on unmount', async () => {
      vi.stubGlobal('ResizeObserver', undefined)
      let width = 1300
      layout(295, () => width, 12 + 1555 + 12)
      const view = await renderSettled(
        <PagedRail perPage={4} gap={20} bleed={12}>
          {cards(5)}
        </PagedRail>
      )
      expect(button('Next')).not.toBeNull()
      width = 1700
      act(() => {
        window.dispatchEvent(new Event('resize'))
      })
      expect(paged()).toBe(false)
      const removed = vi.spyOn(window, 'removeEventListener')
      view.unmount()
      expect(removed.mock.calls.some(([type]) => type === 'resize')).toBe(true)
    })
  })

  describe('when it is drawn without dots', () => {
    it('should keep the arrows', async () => {
      layout(194, 600, 4032)
      await renderSettled(
        <PagedRail perPage={3} gap={8} dots={false}>
          {cards(20)}
        </PagedRail>
      )
      expect(button('Next')).not.toBeNull()
      expect(button('Page 1')).toBeNull()
    })
  })

  describe('when the host passes its arrows and names', () => {
    it("should draw the host's marks in place of the foundations' arrows", async () => {
      layout(194, 600, 4032)
      scrolling(3432)
      await renderSettled(
        <PagedRail
          perPage={3}
          gap={8}
          previousIcon={<i data-testid="back" />}
          nextIcon={<i data-testid="forward" />}
        >
          {cards(20)}
        </PagedRail>
      )
      expect(button('Next')!.querySelector('svg')).toBeNull()
      expect(button('Next')).toContainElement(screen.getByTestId('forward'))
      fireEvent.click(button('Next')!)
      expect(button('Previous')).toContainElement(screen.getByTestId('back'))
    })

    it("should draw the foundations' arrows without them, hidden from assistive technology", async () => {
      layout(194, 600, 4032)
      await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(20)}
        </PagedRail>
      )
      expect(button('Next')!.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
    })

    it("should take the caller's names for the arrows and the dots", async () => {
      layout(194, 600, 4032)
      scrolling(3432)
      await renderSettled(
        <PagedRail
          perPage={3}
          gap={8}
          previousLabel="Anterior"
          nextLabel="Siguiente"
          pageLabel={(page) => `Página ${page}`}
        >
          {cards(20)}
        </PagedRail>
      )
      fireEvent.click(button('Siguiente')!)
      expect(button('Anterior')).not.toBeNull()
      expect(current()).toEqual(['Página 2'])
    })
  })

  describe('when the caller places it', () => {
    it("should carry the caller's class, and the bleed and the gap as custom properties", async () => {
      layout(194, 600, 600)
      const { container } = await renderSettled(
        <PagedRail perPage={3} gap={8} bleed={12} bleedTop={58} variant="shop" className="mine">
          {cards(2)}
        </PagedRail>
      )
      const root = container.firstElementChild as HTMLElement
      expect(root).toHaveClass('mine')
      const viewport = root.firstElementChild as HTMLElement
      expect(viewport.style.getPropertyValue('--rail-bleed')).toBe('12px')
      expect(viewport.style.getPropertyValue('--rail-bleed-top')).toBe('58px')
      const track = viewport.firstElementChild as HTMLElement
      expect(track.style.getPropertyValue('--paged-rail-gap')).toBe('8px')
      expect(track.style.gap).toBe('')
    })

    it('should space the cards by the gap it is handed', () => {
      const sheet = readFileSync(join(__dirname, 'PagedRail.module.css'), 'utf8')
      expect(/(?:^|\n)\.track \{([^}]*)\}/.exec(sheet)?.[1]).toContain('gap: var(--paged-rail-gap);')
    })

    it('should set no bleed without one, and the top bleed alone when only it is given', async () => {
      layout(194, 600, 600)
      const view = await renderSettled(
        <PagedRail perPage={3} gap={8}>
          {cards(2)}
        </PagedRail>
      )
      const viewport = () => view.container.firstElementChild!.firstElementChild as HTMLElement
      expect(viewport().getAttribute('style')).toBeNull()
      await act(async () => {
        view.rerender(
          <PagedRail perPage={3} gap={8} bleedTop={30}>
            {cards(2)}
          </PagedRail>
        )
      })
      expect(viewport().style.getPropertyValue('--rail-bleed')).toBe('0px')
      expect(viewport().style.getPropertyValue('--rail-bleed-top')).toBe('30px')
    })
  })
})
