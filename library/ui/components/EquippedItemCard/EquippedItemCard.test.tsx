import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { EquippedItemCard } from './EquippedItemCard'

// jsdom applies no stylesheet, so what the link's classes do is read from the rules.
const sheet = readFileSync(join(__dirname, 'EquippedItemCard.module.css'), 'utf8')

/** The declarations of the stylesheet's first rule for `selector`, outside any at-rule. */
function rule(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const found = new RegExp(`(?:^|\\n)${escaped} \\{([^}]*)\\}`).exec(sheet)
  if (!found) throw new Error(`no rule ${selector}`)
  return found[1] ?? ''
}

const RARITIES = [
  'common',
  'uncommon',
  'rare',
  'epic',
  'legendary',
  'mythic',
  'unique',
  'exotic'
] as const

const SHOP = 'https://decentraland.org/shop/item/0x0/0'

/** The card: the element that carries the rarity. */
const cardOf = (text: string) => screen.getByText(text).closest('[data-rarity]')

describe('EquippedItemCard', () => {
  describe.each(RARITIES)('when the rarity is %s', (rarity) => {
    it('should mark the card with it and show it as the tag', () => {
      render(<EquippedItemCard name="Hat" rarity={rarity} />)
      expect(cardOf('Hat')).toHaveAttribute('data-rarity', rarity)
      expect(screen.getByText(rarity)).toBeInTheDocument()
    })
  })

  describe('when the card has no rarity', () => {
    it('should be marked as base and say so, so every card of a row has a tag', () => {
      render(<EquippedItemCard name="Plain shirt" />)
      expect(cardOf('Plain shirt')).toHaveAttribute('data-rarity', 'base')
      expect(screen.getByText('base')).toBeInTheDocument()
    })
  })

  describe('when the rarity is one the foundations do not colour', () => {
    it('should show it as given', () => {
      render(<EquippedItemCard name="Odd" rarity="Golden" />)
      expect(cardOf('Odd')).toHaveAttribute('data-rarity', 'Golden')
      expect(screen.getByText('Golden')).toBeInTheDocument()
    })

    it('should keep an empty one empty', () => {
      render(<EquippedItemCard name="Odd" rarity="" />)
      expect(cardOf('Odd')).toHaveAttribute('data-rarity', '')
      expect(screen.queryByText('base')).toBeNull()
    })
  })

  describe('when the thumbnail is missing or fails', () => {
    it('should draw no image without one', () => {
      const { container } = render(<EquippedItemCard name="Hat" />)
      expect(container.querySelector('img')).toBeNull()
    })

    it('should draw no image for an empty one', () => {
      const { container } = render(<EquippedItemCard name="Hat" thumbnail="" />)
      expect(container.querySelector('img')).toBeNull()
    })

    it('should drop an image that fails to load, and keep the caption', () => {
      const { container } = render(<EquippedItemCard name="Hat" rarity="epic" thumbnail="/gone.png" />)
      const image = container.querySelector('img')
      expect(image).toHaveAttribute('src', '/gone.png')
      expect(image).toHaveAttribute('alt', '')
      fireEvent.error(image!)
      expect(container.querySelector('img')).toBeNull()
      expect(screen.getByText('Hat')).toBeInTheDocument()
      expect(screen.getByText('epic')).toBeInTheDocument()
    })

    it('should try a new thumbnail after one failed', () => {
      const { container, rerender } = render(<EquippedItemCard name="Hat" thumbnail="/gone.png" />)
      fireEvent.error(container.querySelector('img')!)
      expect(container.querySelector('img')).toBeNull()
      rerender(<EquippedItemCard name="Hat" thumbnail="/new.png" />)
      expect(container.querySelector('img')).toHaveAttribute('src', '/new.png')
    })
  })

  describe('when the name is long, or missing', () => {
    it('should keep the whole name as its title', () => {
      const name = 'A very long wearable name that no card of 108 pixels could ever show in one line'
      render(<EquippedItemCard name={name} />)
      expect(screen.getByText(name)).toHaveAttribute('title', name)
    })

    it('should render without a name', () => {
      render(<EquippedItemCard rarity="rare" />)
      expect(screen.getByText('rare')).toBeInTheDocument()
    })
  })

  describe('when the item is not on sale', () => {
    it('should show no link and no price', () => {
      render(<EquippedItemCard name="Hat" shopLabel="Buy" />)
      expect(screen.queryByRole('link')).toBeNull()
      expect(screen.queryByText('Buy')).toBeNull()
    })
  })

  describe('when the item is on sale', () => {
    it('should link to its sale in a new tab', () => {
      render(<EquippedItemCard name="Hat" shopUrl={SHOP} />)
      const link = screen.getByRole('link', { name: 'Shop' })
      expect(link).toHaveAttribute('href', SHOP)
      expect(link).toHaveAttribute('target', '_blank')
      expect(link).toHaveAttribute('rel', 'noopener noreferrer')
    })

    it("should take the caller's text for the link, and show the price", () => {
      render(<EquippedItemCard name="Hat" shopUrl={SHOP} shopLabel="Buy" price="25 Credits" />)
      expect(screen.getByRole('link', { name: 'Buy' })).toBeInTheDocument()
      expect(screen.getByText('25 Credits')).toBeInTheDocument()
    })

    it('should show an empty price line as given', () => {
      const { container } = render(<EquippedItemCard name="Hat" price="" />)
      expect(container.querySelectorAll('[data-rarity] > span')).toHaveLength(3)
    })

    it('should hide the link until the card is hovered, when not pinned', () => {
      render(<EquippedItemCard name="Hat" shopUrl={SHOP} />)
      expect(cardOf('Hat')).toHaveClass('hasShop')
      expect(cardOf('Hat')).not.toHaveClass('pinnedShop')
      expect(rule('.shopBtn')).toContain('visibility: hidden;')
      expect(rule('.hasShop:hover .shopBtn')).toContain('visibility: visible;')
    })

    it('should keep the link shown in the card when pinned, reached with Tab', async () => {
      render(<EquippedItemCard name="Hat" shopUrl={SHOP} pinShop />)
      expect(cardOf('Hat')).toHaveClass('pinnedShop')
      expect(cardOf('Hat')).not.toHaveClass('hasShop')
      expect(rule('.pinnedShop .shopBtn')).toContain('position: static;')
      expect(rule('.pinnedShop .shopBtn')).toContain('visibility: visible;')
      await userEvent.tab()
      expect(screen.getByRole('link', { name: 'Shop' })).toHaveFocus()
    })

    it('should ignore the pin without a sale', () => {
      render(<EquippedItemCard name="Hat" pinShop />)
      expect(screen.queryByRole('link')).toBeNull()
    })
  })

  describe('when it has a category glyph', () => {
    it('should draw it', () => {
      render(<EquippedItemCard name="Hat" categoryIcon={<svg data-testid="glyph" />} />)
      expect(screen.getByTestId('glyph')).toBeInTheDocument()
    })
  })

  describe('when the passport restyles it by element', () => {
    it('should keep its elements: a picture block, then the name, the tag and the price', () => {
      const { container } = render(
        <EquippedItemCard name="Hat" rarity="epic" price="25 Credits" shopUrl={SHOP} />
      )
      const card = container.firstElementChild!
      expect(card.tagName).toBe('DIV')
      expect([...card.children].map((child) => child.tagName)).toEqual(['DIV', 'SPAN', 'SPAN', 'SPAN', 'A'])
      expect([...card.children].map((child) => child.textContent)).toEqual([
        '',
        'Hat',
        'epic',
        '25 Credits',
        'Shop'
      ])
    })
  })

  describe('when the card has an action', () => {
    it('should make its picture a named button that runs it, and run it for a click anywhere on the card', async () => {
      const onAction = vi.fn()
      render(<EquippedItemCard name="Disco" onAction={onAction} actionLabel="Play Disco" />)
      await userEvent.click(screen.getByRole('button', { name: 'Play Disco' }))
      expect(onAction).toHaveBeenCalledTimes(1)
      await userEvent.click(screen.getByText('Disco'))
      expect(onAction).toHaveBeenCalledTimes(2)
    })

    it('should not run it for a click on the link to the sale', async () => {
      const onAction = vi.fn()
      render(<EquippedItemCard name="Disco" shopUrl={SHOP} onAction={onAction} actionLabel="Play Disco" />)
      const link = screen.getByRole('link', { name: 'Shop' })
      link.addEventListener('click', (e) => e.preventDefault())
      await userEvent.click(link)
      expect(onAction).not.toHaveBeenCalled()
    })

    it('should have no button without one', () => {
      render(<EquippedItemCard name="Hat" />)
      expect(screen.queryByRole('button')).toBeNull()
    })
  })
})
