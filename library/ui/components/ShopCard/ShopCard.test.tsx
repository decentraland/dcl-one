import { readFileSync } from 'node:fs'
import type { ReactElement } from 'react'
import { join } from 'node:path'
import { fireEvent, render, screen, within, type RenderResult } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { ShopCard, ShopCardPrice, ShopCardSkeleton, type ShopCardLinkProps } from './ShopCard'

const item = { name: 'Golden Armor', image: 'https://example.org/armor.png', rarity: 'epic' }
const setup = (props: Partial<Parameters<typeof ShopCard>[0]> = {}) =>
  render(<ShopCard {...item} {...props} />)

describe('ShopCard', () => {
  let container: HTMLElement

  afterEach(() => {
    vi.restoreAllMocks()
    vi.resetAllMocks()
  })

  describe('when rendered with an item', () => {
    beforeEach(() => {
      ;({ container } = setup())
    })

    it('should show its name', () => {
      expect(screen.getByText('Golden Armor')).toBeInTheDocument()
    })

    it('should show its rarity label', () => {
      expect(screen.getByText('epic')).toBeInTheDocument()
    })

    it('should show its image', () => {
      expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.org/armor.png')
    })

    it('should tint the card by its rarity', () => {
      expect(screen.getByRole('article')).toHaveAttribute('data-rarity', 'epic')
    })
  })

  describe('when rendered with a creator', () => {
    beforeEach(() => {
      setup({
        creator: { name: 'Alice', picture: 'https://example.org/alice.png', color: '#ff8362' }
      })
    })

    it("should show the creator's colour", () => {
      expect(screen.getByText('Alice').style.getPropertyValue('--creator-color')).toBe('#ff8362')
    })

    it("should show the creator's face", () => {
      expect(screen.getByText('Alice').previousElementSibling).toHaveAttribute(
        'src',
        'https://example.org/alice.png'
      )
    })
  })

  describe('when rendered with a price, badges, a category glyph and a discount', () => {
    beforeEach(() => {
      setup({
        price: <ShopCardPrice kind="credits" amount="120" />,
        badges: <span role="img" aria-label="Unisex" />,
        categoryIcon: <svg data-testid="glyph" />,
        discount: '-20%'
      })
    })

    it('should show the price', () => {
      expect(screen.getByText(/Price:/).parentElement).toHaveTextContent('Price: 120 credits')
    })

    it('should show the badges', () => {
      expect(screen.getByRole('img', { name: 'Unisex' })).toBeInTheDocument()
    })

    it('should show the category glyph', () => {
      expect(screen.getByTestId('glyph')).toBeInTheDocument()
    })

    it('should show the discount', () => {
      expect(screen.getByText('-20%')).toBeInTheDocument()
    })
  })

  describe('when the rarity is missing', () => {
    beforeEach(() => {
      setup({ rarity: undefined })
    })

    it('should draw the card as base', () => {
      expect(screen.getByRole('article')).toHaveAttribute('data-rarity', 'base')
    })

    it('should show no rarity label', () => {
      expect(screen.queryByText('epic')).toBeNull()
    })
  })

  describe('when the rarity is unknown', () => {
    beforeEach(() => {
      setup({ rarity: 'legacy' })
    })

    it('should draw the card as base', () => {
      expect(screen.getByRole('article')).toHaveAttribute('data-rarity', 'base')
    })

    it('should still name the rarity', () => {
      expect(screen.getByText('legacy')).toBeInTheDocument()
    })
  })

  describe('when the image is missing or fails', () => {
    describe('and there is no image', () => {
      beforeEach(() => {
        ;({ container } = setup({
          image: undefined,
          placeholder: <svg data-testid="name-glyph" />
        }))
      })

      it('should render no image', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      it('should show the placeholder', () => {
        expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
      })
    })

    describe('and the image fails with a placeholder', () => {
      beforeEach(() => {
        ;({ container } = setup({ placeholder: <svg data-testid="name-glyph" /> }))
        fireEvent.error(container.querySelector('img') as HTMLImageElement)
      })

      it('should drop the failed image', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      it('should show the placeholder', () => {
        expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
      })
    })

    describe('and the image fails without a placeholder', () => {
      let rerender: RenderResult['rerender']

      beforeEach(() => {
        ;({ container, rerender } = setup())
        fireEvent.error(container.querySelector('img') as HTMLImageElement)
      })

      it('should leave the thumbnail empty', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      describe('and a new image is given', () => {
        beforeEach(() => {
          rerender(<ShopCard {...item} image="https://example.org/armor-2.png" />)
        })

        it('should try the new image', () => {
          expect(container.querySelector('img')).toHaveAttribute(
            'src',
            'https://example.org/armor-2.png'
          )
        })
      })
    })

    describe('and the image failed before the page hydrated', () => {
      beforeEach(() => {
        vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true)
        vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(0)
        ;({ container } = setup({ placeholder: <svg data-testid="name-glyph" /> }))
      })

      it('should drop the image', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      it('should show the placeholder', () => {
        expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
      })
    })

    describe('and the creator has no picture', () => {
      beforeEach(() => {
        ;({ container } = setup({ creator: { name: 'Alice' } }))
      })

      it('should show the blank face', () => {
        expect(container.querySelectorAll('img')).toHaveLength(1)
      })
    })

    describe('and the creator picture fails', () => {
      beforeEach(() => {
        ;({ container } = setup({
          creator: { name: 'Alice', picture: 'https://example.org/alice.png' }
        }))
        fireEvent.error(screen.getByText('Alice').previousElementSibling as HTMLImageElement)
      })

      it('should drop the creator picture', () => {
        expect(container.querySelectorAll('img')).toHaveLength(1)
      })

      it('should show the blank face', () => {
        expect(screen.getByText('Alice').previousElementSibling?.tagName).toBe('SPAN')
      })
    })
  })

  describe('when an image URL is not safe to load', () => {
    describe.each([
      'http://example.org/armor.png',
      'javascript:alert(1)',
      '//example.org/armor.png',
      'https://u:p@example.org/a.png'
    ])('and the thumbnail is %s', (image) => {
      beforeEach(() => {
        ;({ container } = setup({ image, placeholder: <svg data-testid="name-glyph" /> }))
      })

      it('should drop the thumbnail for the placeholder', () => {
        expect(container.querySelector('img')).toBeNull()
        expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
      })
    })

    describe('and the thumbnail is root-relative', () => {
      beforeEach(() => {
        ;({ container } = setup({ image: '/images/armor.png' }))
      })

      it('should keep the thumbnail', () => {
        expect(container.querySelector('img')).toHaveAttribute('src', '/images/armor.png')
      })
    })

    describe('and the creator picture is unsafe', () => {
      beforeEach(() => {
        ;({ container } = setup({
          creator: { name: 'Alice', picture: 'http://example.org/alice.png' }
        }))
      })

      it('should draw the blank face', () => {
        expect(container.querySelectorAll('img')).toHaveLength(1)
      })
    })
  })

  describe('when it has an href', () => {
    describe('and no heading level', () => {
      beforeEach(() => {
        setup({ href: '/shop/item/0xabc/1' })
      })

      it('should link the whole card, named by the item', () => {
        expect(screen.getByRole('link', { name: 'Golden Armor' })).toHaveAttribute(
          'href',
          '/shop/item/0xabc/1'
        )
      })

      it('should render no button', () => {
        expect(screen.queryByRole('button')).toBeNull()
      })
    })

    describe('and a heading level', () => {
      beforeEach(() => {
        ;({ container } = setup({ href: '/shop/item/0xabc/1', headingLevel: 3 }))
      })

      it('should make the name the link, inside the heading', () => {
        const heading = screen.getByRole('heading', { level: 3, name: 'Golden Armor' })
        expect(within(heading).getByRole('link', { name: 'Golden Armor' })).toBeInTheDocument()
      })

      it('should keep one link in the tab order', () => {
        expect(screen.getAllByRole('link')).toHaveLength(1)
      })

      it('should add the same link as a hidden, empty full-card pointer target', () => {
        // The full-card pointer target: the same link, out of the tab order and the accessibility tree.
        const anchors = container.querySelectorAll('a')
        expect(anchors).toHaveLength(2)
        const target = [...anchors].find((a) => a.getAttribute('aria-hidden') === 'true')
        expect(target).toHaveAttribute('tabindex', '-1')
        expect(target).toHaveAttribute('href', '/shop/item/0xabc/1')
        expect(target).toBeEmptyDOMElement()
      })
    })

    describe('and a heading level and actions', () => {
      beforeEach(() => {
        setup({
          href: '/shop/item/0xabc/1',
          headingLevel: 3,
          actions: <button type="button">Buy</button>
        })
      })

      it('should put the heading before the link it holds, so a heading jump lands on the card link', () => {
        const link = screen.getByRole('link', { name: 'Golden Armor' })
        const buy = screen.getByRole('button', { name: 'Buy' })
        expect(link.compareDocumentPosition(buy) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
      })
    })

    describe('and a router link component', () => {
      let RouterLink: Mock<(props: ShopCardLinkProps) => ReactElement>

      beforeEach(() => {
        RouterLink = vi.fn(({ href, ...rest }: ShopCardLinkProps) => (
          <a data-router="" href={href} {...rest} />
        ))
        setup({ href: '/shop/item/0xabc/1', linkComponent: RouterLink })
      })

      it('should render the link through the router link', () => {
        expect(screen.getByRole('link', { name: 'Golden Armor' })).toHaveAttribute('data-router')
        expect(RouterLink).toHaveBeenCalled()
      })
    })
  })

  describe('when it is selectable', () => {
    let onSelect: Mock<() => void>

    beforeEach(() => {
      onSelect = vi.fn()
    })

    describe('and it is selected', () => {
      let hit: HTMLElement

      beforeEach(() => {
        setup({ onSelect, selected: true })
        hit = screen.getByRole('button', { name: 'Golden Armor' })
      })

      it('should be a pressed toggle button', () => {
        expect(hit).toHaveAttribute('aria-pressed', 'true')
      })

      it('should report a press', () => {
        fireEvent.click(hit)
        expect(onSelect).toHaveBeenCalledOnce()
      })
    })

    describe('and the card is pressed anywhere', () => {
      let target: HTMLButtonElement

      beforeEach(() => {
        ;({ container } = setup({ onSelect }))
        target = container.querySelector('button[aria-hidden="true"]') as HTMLButtonElement
      })

      it('should expose a single button', () => {
        expect(screen.getAllByRole('button')).toHaveLength(1)
      })

      it('should keep the pointer target out of the tab order', () => {
        expect(target).toHaveAttribute('tabindex', '-1')
      })

      it('should toggle through the hidden pointer target', () => {
        fireEvent.click(target)
        expect(onSelect).toHaveBeenCalledOnce()
      })
    })

    describe('and it also has an href', () => {
      beforeEach(() => {
        setup({ href: '/shop/item/0xabc/1', onSelect, selected: true })
      })

      it('should link rather than toggle', () => {
        expect(screen.getByRole('link', { name: 'Golden Armor' })).not.toHaveAttribute(
          'aria-pressed'
        )
        expect(screen.queryByRole('button')).toBeNull()
      })
    })
  })

  describe('when it has neither onSelect nor href', () => {
    describe('and it is selected', () => {
      beforeEach(() => {
        setup({ selected: true })
      })

      it('should have no control', () => {
        expect(screen.queryByRole('button')).toBeNull()
      })
    })

    describe('and it is not selected', () => {
      beforeEach(() => {
        setup()
      })

      it('should have no button', () => {
        expect(screen.queryByRole('button')).toBeNull()
      })

      it('should have no link', () => {
        expect(screen.queryByRole('link')).toBeNull()
      })
    })
  })

  describe('when it has a heading level', () => {
    beforeEach(() => {
      setup({ headingLevel: 3 })
    })

    it('should name the card by a heading of that level', () => {
      expect(screen.getByRole('heading', { level: 3, name: 'Golden Armor' })).toBeInTheDocument()
    })
  })

  describe('when it has no heading level', () => {
    beforeEach(() => {
      setup()
    })

    it('should have no heading', () => {
      expect(screen.queryByRole('heading')).toBeNull()
    })
  })

  describe('when it has actions', () => {
    beforeEach(() => {
      setup({ actions: <button type="button">Buy</button> })
    })

    it('should render them in the card, reachable by keyboard', () => {
      expect(screen.getByRole('button', { name: 'Buy' })).toBeInTheDocument()
    })
  })
})

describe('ShopCardPrice', () => {
  describe('when the price is in MANA on Ethereum', () => {
    beforeEach(() => {
      render(<ShopCardPrice kind="mana" amount="69,069" network="ethereum" />)
    })

    it('should name the MANA price', () => {
      expect(screen.getByText(/Price:/).parentElement).toHaveTextContent('Price: 69,069 MANA')
    })
  })

  describe('when there is no price', () => {
    beforeEach(() => {
      render(<ShopCardPrice kind="none" label="Not for sale" />)
    })

    it('should show the reason', () => {
      expect(screen.getByText('Not for sale')).toBeInTheDocument()
    })

    it('should show no amount', () => {
      expect(screen.queryByText(/Price:/)).toBeNull()
    })
  })
})

describe('ShopCardSkeleton', () => {
  let container: HTMLElement

  describe('when rendered', () => {
    beforeEach(() => {
      ;({ container } = render(<ShopCardSkeleton />))
    })

    it('should be hidden from assistive technology', () => {
      expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
    })
  })
})

/** The module's text, and the declarations of the rule whose selector list is exactly `selector`. */
// Vitest runs from library/ui (its config's root), where jsdom gives no file URL for this module.
const css = readFileSync(join(process.cwd(), 'components/ShopCard/ShopCard.module.css'), 'utf8')
const blockOf = (source: string, selector: string): string => {
  const start = source.indexOf(`${selector} {`)
  if (start < 0) throw new Error(`no rule ${selector}`)
  return source.slice(start, source.indexOf('}', start))
}
const mediaOf = (query: string): string => {
  const start = css.indexOf(`@media (${query}) {`)
  if (start < 0) throw new Error(`no @media (${query})`)
  // The block ends at the first line that closes it at its own indentation.
  return css.slice(start, css.indexOf('\n  }\n', start))
}
const zIndex = (block: string) => Number(/z-index: (\d+)/.exec(block)?.[1])

describe('ShopCard motion', () => {
  describe('when the card is a catalogue card with actions', () => {
    beforeEach(() => {
      render(
        <ShopCard {...item} variant="catalogue" actions={<button type="button">Buy</button>} />
      )
    })

    it('should key the catalogue motion on its class, and the swap on having actions', () => {
      expect(screen.getByRole('article')).toHaveClass('catalogue', 'hasActions')
    })
  })

  describe('when the card is an in-world Shop card', () => {
    beforeEach(() => {
      render(<ShopCard {...item} actions={<button type="button">Buy</button>} />)
    })

    it('should give it no catalogue motion', () => {
      expect(screen.getByRole('article')).not.toHaveClass('catalogue')
    })

    it('should only fade its ring, as the in-world card did', () => {
      // The shop variant only fades its ring (140ms), as the in-world card did.
      expect(blockOf(css, '.card::after')).toContain('transition: opacity 140ms')
      expect(css).not.toMatch(/\.card:hover\s*\{[^}]*transform/)
    })
  })

  describe('when a catalogue card is hovered or keyboard-focused', () => {
    it('should slide the actions up from the bottom and back', () => {
      const rest = blockOf(css, '.catalogue .actions')
      expect(rest).toContain('transform: translateY(100%)')
      expect(rest).toMatch(/opacity 300ms ease-in-out,\s*transform 300ms ease-in-out/)
      const shown = blockOf(
        css,
        '.catalogue.hasActions:hover .actions,\n  .catalogue.hasActions:has(:focus-visible) .actions'
      )
      expect(shown).toContain('transform: translateY(0)')
      expect(
        blockOf(css, '.hasActions:hover .actions,\n  .hasActions:has(:focus-visible) .actions')
      ).toContain('opacity: 1')
    })

    it('should lift the card and bring its ring out from behind its edge', () => {
      expect(blockOf(css, '.catalogue')).toContain('transition: transform 200ms ease-in-out')
      expect(blockOf(css, '.catalogue:hover,\n  .catalogue:has(:focus-visible)')).toContain(
        'transform: translateY(-4px)'
      )
      // A mouse click's focus (e.g. Cmd-click to a new tab) leaves the card down, as it leaves the ring off.
      expect(css).not.toContain(':focus-within')
      expect(blockOf(css, '.catalogue::after')).toMatch(/inset: 0;[\s\S]*inset 300ms ease-in-out/)
      expect(
        blockOf(
          css,
          '.catalogue:hover::after,\n  .catalogue:has(:focus-visible)::after,\n  .catalogue.selected::after'
        )
      ).toContain('inset: -8px')
    })
  })

  describe('when the user prefers reduced motion', () => {
    let reduced: string

    beforeEach(() => {
      reduced = mediaOf('prefers-reduced-motion: reduce')
    })

    it('should show every state at once, in place', () => {
      expect(
        blockOf(
          reduced,
          '.card::after,\n    .catalogue,\n    .catalogue .actions,\n    .catalogue .creator,\n    .catalogue .meta'
        )
      ).toContain('transition: none')
      expect(
        blockOf(
          reduced,
          '.catalogue:hover,\n    .catalogue:has(:focus-visible),\n    .catalogue .actions'
        )
      ).toContain('transform: none')
      expect(reduced).toContain('animation: none')
    })
  })

  describe('when the card is on a touch screen', () => {
    let touch: string

    beforeEach(() => {
      touch = mediaOf('hover: none')
    })

    it('should neither lift nor ring a tapped card unless it is focused or selected', () => {
      expect(blockOf(touch, '.catalogue:hover:not(:has(:focus-visible))')).toContain(
        'transform: none'
      )
      const ring = blockOf(touch, '.catalogue:hover:not(.selected, :has(:focus-visible))::after')
      expect(ring).toContain('opacity: 0')
      expect(ring).toContain('inset: 0')
    })

    it('should keep the actions shown and unmoved', () => {
      const actions = blockOf(touch, '.catalogue .actions')
      expect(actions).toContain('opacity: 1')
      expect(actions).toContain('pointer-events: auto')
      expect(actions).toContain('transform: none')
    })

    it('should keep the actions above the full-card link, so a tap reaches them', () => {
      const actions = blockOf(touch, '.catalogue .actions')
      expect(actions).toContain('position: relative')
      expect(actions).not.toContain('position: static')
      // In the flow, the hover layout's right, bottom and left offsets would shift the row and overlap the price.
      expect(actions).toContain('inset: auto')
      expect(zIndex(actions)).toBeGreaterThan(zIndex(blockOf(css, '.hit')))
    })
  })

  describe('when forced colours drop the ring', () => {
    it('should outline keyboard focus', () => {
      expect(blockOf(mediaOf('forced-colors: active'), '.control:focus-visible')).toContain(
        'outline: 2px solid CanvasText'
      )
    })
  })

  describe('when the name and badges sit over the full-card target', () => {
    it('should raise them above the target, so their tooltips show', () => {
      const hit = zIndex(blockOf(css, '.hit'))
      expect(zIndex(blockOf(css, '.name'))).toBeGreaterThan(hit)
      expect(zIndex(blockOf(css, '.badges'))).toBeGreaterThan(hit)
      expect(blockOf(css, '.name')).toContain('pointer-events: none')
      expect(blockOf(css, '.control')).toContain('pointer-events: auto')
    })
  })
})
