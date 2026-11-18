import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ShopCard, ShopCardPrice, ShopCardSkeleton, type ShopCardLinkProps } from './ShopCard'

const item = { name: 'Golden Armor', image: 'https://example.org/armor.png', rarity: 'epic' }
const setup = (props: Partial<Parameters<typeof ShopCard>[0]> = {}) =>
  render(<ShopCard {...item} {...props} />)

describe('ShopCard', () => {
  describe('when rendered with an item', () => {
    it('shows its name, rarity label and image', () => {
      const { container } = setup()
      expect(screen.getByText('Golden Armor')).toBeInTheDocument()
      expect(screen.getByText('epic')).toBeInTheDocument()
      expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.org/armor.png')
    })

    it('tints the card by its rarity', () => {
      setup()
      expect(screen.getByRole('article')).toHaveAttribute('data-rarity', 'epic')
    })

    it('shows the creator with their face and colour', () => {
      setup({ creator: { name: 'Alice', picture: 'https://example.org/alice.png', color: '#ff8362' } })
      const creator = screen.getByText('Alice')
      expect(creator.style.getPropertyValue('--creator-color')).toBe('#ff8362')
      expect(creator.previousElementSibling).toHaveAttribute('src', 'https://example.org/alice.png')
    })

    it('shows the price, badges, category glyph and discount it is given', () => {
      setup({
        price: <ShopCardPrice kind="credits" amount="120" />,
        badges: <span role="img" aria-label="Unisex" />,
        categoryIcon: <svg data-testid="glyph" />,
        discount: '-20%'
      })
      expect(screen.getByText(/Price:/).parentElement).toHaveTextContent('Price: 120 credits')
      expect(screen.getByRole('img', { name: 'Unisex' })).toBeInTheDocument()
      expect(screen.getByTestId('glyph')).toBeInTheDocument()
      expect(screen.getByText('-20%')).toBeInTheDocument()
    })
  })

  describe('when the rarity is missing or unknown', () => {
    it('draws the card as base without a label', () => {
      setup({ rarity: undefined })
      expect(screen.getByRole('article')).toHaveAttribute('data-rarity', 'base')
      expect(screen.queryByText('epic')).toBeNull()
    })

    it('draws an unknown rarity as base and still names it', () => {
      setup({ rarity: 'legacy' })
      expect(screen.getByRole('article')).toHaveAttribute('data-rarity', 'base')
      expect(screen.getByText('legacy')).toBeInTheDocument()
    })
  })

  describe('when the image is missing or fails', () => {
    it('shows the placeholder without an image', () => {
      const { container } = setup({ image: undefined, placeholder: <svg data-testid="name-glyph" /> })
      expect(container.querySelector('img')).toBeNull()
      expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
    })

    it('drops a failed image for the placeholder', () => {
      const { container } = setup({ placeholder: <svg data-testid="name-glyph" /> })
      fireEvent.error(container.querySelector('img') as HTMLImageElement)
      expect(container.querySelector('img')).toBeNull()
      expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
    })

    it('leaves the thumbnail empty when a failed image has no placeholder', () => {
      const { container } = setup()
      fireEvent.error(container.querySelector('img') as HTMLImageElement)
      expect(container.querySelector('img')).toBeNull()
    })

    it('tries a new image after the previous one failed', () => {
      const { container, rerender } = setup()
      fireEvent.error(container.querySelector('img') as HTMLImageElement)
      rerender(<ShopCard {...item} image="https://example.org/armor-2.png" />)
      expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.org/armor-2.png')
    })

    it('drops an image that failed before the page hydrated', () => {
      const complete = vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true)
      const width = vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(0)
      const { container } = setup({ placeholder: <svg data-testid="name-glyph" /> })
      expect(container.querySelector('img')).toBeNull()
      expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
      complete.mockRestore()
      width.mockRestore()
    })

    it('shows the blank face when the creator has no picture', () => {
      const { container } = setup({ creator: { name: 'Alice' } })
      expect(container.querySelectorAll('img')).toHaveLength(1)
    })

    it('shows the blank face when the creator picture fails', () => {
      const { container } = setup({ creator: { name: 'Alice', picture: 'https://example.org/alice.png' } })
      const face = screen.getByText('Alice').previousElementSibling as HTMLImageElement
      fireEvent.error(face)
      expect(container.querySelectorAll('img')).toHaveLength(1)
      expect(screen.getByText('Alice').previousElementSibling?.tagName).toBe('SPAN')
    })
  })

  describe('when an image URL is not safe to load', () => {
    it.each(['http://example.org/armor.png', 'javascript:alert(1)', '//example.org/armor.png', 'https://u:p@example.org/a.png'])(
      'drops the thumbnail %s for the placeholder',
      (image) => {
        const { container } = setup({ image, placeholder: <svg data-testid="name-glyph" /> })
        expect(container.querySelector('img')).toBeNull()
        expect(screen.getByTestId('name-glyph')).toBeInTheDocument()
      }
    )

    it('keeps a root-relative thumbnail', () => {
      const { container } = setup({ image: '/images/armor.png' })
      expect(container.querySelector('img')).toHaveAttribute('src', '/images/armor.png')
    })

    it('draws the blank face for an unsafe creator picture', () => {
      const { container } = setup({ creator: { name: 'Alice', picture: 'http://example.org/alice.png' } })
      expect(container.querySelectorAll('img')).toHaveLength(1)
    })
  })

  describe('when it has an href', () => {
    it('links the whole card, named by the item', () => {
      setup({ href: '/shop/item/0xabc/1' })
      expect(screen.getByRole('link', { name: 'Golden Armor' })).toHaveAttribute(
        'href',
        '/shop/item/0xabc/1'
      )
      expect(screen.queryByRole('button')).toBeNull()
    })

    it('makes the name the link, inside the heading, with one link in the tab order', () => {
      const { container } = setup({ href: '/shop/item/0xabc/1', headingLevel: 3 })
      const heading = screen.getByRole('heading', { level: 3, name: 'Golden Armor' })
      expect(within(heading).getByRole('link', { name: 'Golden Armor' })).toBeInTheDocument()
      expect(screen.getAllByRole('link')).toHaveLength(1)
      // The full-card pointer target: the same link, out of the tab order and the accessibility tree.
      const anchors = container.querySelectorAll('a')
      expect(anchors).toHaveLength(2)
      const target = [...anchors].find((a) => a.getAttribute('aria-hidden') === 'true')
      expect(target).toHaveAttribute('tabindex', '-1')
      expect(target).toHaveAttribute('href', '/shop/item/0xabc/1')
      expect(target).toBeEmptyDOMElement()
    })

    it('puts the heading before the link it holds, so a heading jump lands on the card link', () => {
      setup({ href: '/shop/item/0xabc/1', headingLevel: 3, actions: <button type="button">Buy</button> })
      const link = screen.getByRole('link', { name: 'Golden Armor' })
      const buy = screen.getByRole('button', { name: 'Buy' })
      expect(link.compareDocumentPosition(buy) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    })

    it('renders the link through the router link it is given', () => {
      const RouterLink = vi.fn(({ href, ...rest }: ShopCardLinkProps) => (
        <a data-router="" href={href} {...rest} />
      ))
      setup({ href: '/shop/item/0xabc/1', linkComponent: RouterLink })
      expect(screen.getByRole('link', { name: 'Golden Armor' })).toHaveAttribute('data-router')
      expect(RouterLink).toHaveBeenCalled()
    })
  })

  describe('when it is selectable', () => {
    it('is a toggle button that reports a press', () => {
      const onSelect = vi.fn()
      setup({ onSelect, selected: true })
      const hit = screen.getByRole('button', { name: 'Golden Armor' })
      expect(hit).toHaveAttribute('aria-pressed', 'true')
      fireEvent.click(hit)
      expect(onSelect).toHaveBeenCalledOnce()
    })

    it('toggles from anywhere on the card through its pointer target, which is hidden', () => {
      const onSelect = vi.fn()
      const { container } = setup({ onSelect })
      expect(screen.getAllByRole('button')).toHaveLength(1)
      const target = container.querySelector('button[aria-hidden="true"]') as HTMLButtonElement
      expect(target).toHaveAttribute('tabindex', '-1')
      fireEvent.click(target)
      expect(onSelect).toHaveBeenCalledOnce()
    })

    it('links rather than toggles when it has both href and onSelect', () => {
      setup({ href: '/shop/item/0xabc/1', onSelect: vi.fn(), selected: true })
      expect(screen.getByRole('link', { name: 'Golden Armor' })).not.toHaveAttribute('aria-pressed')
      expect(screen.queryByRole('button')).toBeNull()
    })

    it('has no control without onSelect or href, even when selected', () => {
      setup({ selected: true })
      expect(screen.queryByRole('button')).toBeNull()
    })

    it('has no control without onSelect or href', () => {
      setup()
      expect(screen.queryByRole('button')).toBeNull()
      expect(screen.queryByRole('link')).toBeNull()
    })
  })

  describe('when it has a heading level', () => {
    it('names the card by a heading of that level', () => {
      setup({ headingLevel: 3 })
      expect(screen.getByRole('heading', { level: 3, name: 'Golden Armor' })).toBeInTheDocument()
    })

    it('has no heading without one', () => {
      setup()
      expect(screen.queryByRole('heading')).toBeNull()
    })
  })

  describe('when it has actions', () => {
    it('renders them in the card, reachable by keyboard', () => {
      setup({ actions: <button type="button">Buy</button> })
      expect(screen.getByRole('button', { name: 'Buy' })).toBeInTheDocument()
    })
  })
})

describe('ShopCardPrice', () => {
  it('names a MANA price on Ethereum', () => {
    render(<ShopCardPrice kind="mana" amount="69,069" network="ethereum" />)
    expect(screen.getByText(/Price:/).parentElement).toHaveTextContent('Price: 69,069 MANA')
  })

  it('shows the reason there is no price, with no amount', () => {
    render(<ShopCardPrice kind="none" label="Not for sale" />)
    expect(screen.getByText('Not for sale')).toBeInTheDocument()
    expect(screen.queryByText(/Price:/)).toBeNull()
  })
})

describe('ShopCardSkeleton', () => {
  it('is hidden from assistive technology', () => {
    const { container } = render(<ShopCardSkeleton />)
    expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
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

describe('ShopCard motion', () => {
  it('keys the catalogue motion on its class, and the swap on having actions', () => {
    render(<ShopCard {...item} variant="catalogue" actions={<button type="button">Buy</button>} />)
    const card = screen.getByRole('article')
    expect(card).toHaveClass('catalogue', 'hasActions')
  })

  it('gives the in-world Shop card no catalogue motion', () => {
    render(<ShopCard {...item} actions={<button type="button">Buy</button>} />)
    expect(screen.getByRole('article')).not.toHaveClass('catalogue')
    // The shop variant only fades its ring (140ms), as the in-world card did.
    expect(blockOf(css, '.card::after')).toContain('transition: opacity 140ms')
    expect(css).not.toMatch(/\.card:hover\s*\{[^}]*transform/)
  })

  it('slides the actions up from the bottom and back on hover and keyboard focus', () => {
    const rest = blockOf(css, '.catalogue .actions')
    expect(rest).toContain('transform: translateY(100%)')
    expect(rest).toMatch(/opacity 300ms ease-in-out,\s*transform 300ms ease-in-out/)
    const shown = blockOf(
      css,
      '.catalogue.hasActions:hover .actions,\n  .catalogue.hasActions:has(:focus-visible) .actions'
    )
    expect(shown).toContain('transform: translateY(0)')
    expect(blockOf(css, '.hasActions:hover .actions,\n  .hasActions:has(:focus-visible) .actions')).toContain(
      'opacity: 1'
    )
  })

  it('lifts the card and brings its ring out from behind its edge', () => {
    expect(blockOf(css, '.catalogue')).toContain('transition: transform 200ms ease-in-out')
    expect(blockOf(css, '.catalogue:hover,\n  .catalogue:has(:focus-visible)')).toContain('transform: translateY(-4px)')
    // A mouse click's focus (e.g. Cmd-click to a new tab) leaves the card down, as it leaves the ring off.
    expect(css).not.toContain(':focus-within')
    expect(blockOf(css, '.catalogue::after')).toMatch(/inset: 0;[\s\S]*inset 300ms ease-in-out/)
    expect(
      blockOf(css, '.catalogue:hover::after,\n  .catalogue:has(:focus-visible)::after,\n  .catalogue.selected::after')
    ).toContain('inset: -8px')
  })

  it('shows every state at once, in place, under reduced motion', () => {
    const reduced = mediaOf('prefers-reduced-motion: reduce')
    expect(blockOf(reduced, '.card::after,\n    .catalogue,\n    .catalogue .actions,\n    .catalogue .creator,\n    .catalogue .meta')).toContain(
      'transition: none'
    )
    expect(blockOf(reduced, '.catalogue:hover,\n    .catalogue:has(:focus-visible),\n    .catalogue .actions')).toContain(
      'transform: none'
    )
    expect(reduced).toContain('animation: none')
  })

  it('neither lifts nor rings a tapped card on a touch screen unless it is focused or selected', () => {
    const touch = mediaOf('hover: none')
    expect(blockOf(touch, '.catalogue:hover:not(:has(:focus-visible))')).toContain('transform: none')
    const ring = blockOf(touch, '.catalogue:hover:not(.selected, :has(:focus-visible))::after')
    expect(ring).toContain('opacity: 0')
    expect(ring).toContain('inset: 0')
  })

  it('outlines keyboard focus in forced colours, which drop the ring', () => {
    expect(blockOf(mediaOf('forced-colors: active'), '.control:focus-visible')).toContain('outline: 2px solid CanvasText')
  })

  it('raises the name and the badges above the full-card target, so their tooltips show', () => {
    const z = (block: string) => Number(/z-index: (\d+)/.exec(block)?.[1])
    const hit = z(blockOf(css, '.hit'))
    expect(z(blockOf(css, '.name'))).toBeGreaterThan(hit)
    expect(z(blockOf(css, '.badges'))).toBeGreaterThan(hit)
    expect(blockOf(css, '.name')).toContain('pointer-events: none')
    expect(blockOf(css, '.control')).toContain('pointer-events: auto')
  })

  it('keeps the actions shown and unmoved on a touch screen', () => {
    const touch = mediaOf('hover: none')
    const actions = blockOf(touch, '.catalogue .actions')
    expect(actions).toContain('opacity: 1')
    expect(actions).toContain('pointer-events: auto')
    expect(actions).toContain('transform: none')
  })

  it('keeps the touch actions above the full-card link, so a tap reaches them', () => {
    const zIndex = (block: string) => Number(/z-index: (\d+)/.exec(block)?.[1])
    const actions = blockOf(mediaOf('hover: none'), '.catalogue .actions')
    expect(actions).toContain('position: relative')
    expect(actions).not.toContain('position: static')
    // In the flow, the hover layout's right, bottom and left offsets would shift the row and overlap the price.
    expect(actions).toContain('inset: auto')
    expect(zIndex(actions)).toBeGreaterThan(zIndex(blockOf(css, '.hit')))
  })
})
