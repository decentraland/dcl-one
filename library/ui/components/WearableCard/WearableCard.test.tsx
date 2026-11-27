import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { WearableCard, type WearableCardRarity } from './WearableCard'

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

// jsdom resolves no custom property through a stylesheet, so the tones are read from the rules.
const tones = readFileSync(join(__dirname, 'rarity.module.css'), 'utf8')
const colours = readFileSync(join(__dirname, '../../foundations/colors/colors.css'), 'utf8')

function tone(selector: string): string {
  const rule = [...tones.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
    ([, found]) => (found ?? '').replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector
  )
  if (!rule) throw new Error(`no rule ${selector}`)
  const value = /--item-rarity:\s*var\((--dcl-[\w-]+)\);/.exec(rule[2] ?? '')?.[1]
  if (!value) throw new Error(`${selector} sets no foundation token`)
  return value
}

const sheet = readFileSync(join(__dirname, 'WearableCard.module.css'), 'utf8')

/** The declarations of the card stylesheet's top-level rule for `selector`. */
function rule(selector: string): string {
  const found = [...sheet.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
    ([, head]) => (head ?? '').replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector
  )
  if (!found) throw new Error(`no rule ${selector}`)
  return found[2] ?? ''
}

const card = (name: string) => screen.getByRole('button', { name })

describe('WearableCard', () => {
  describe.each(RARITIES)('when the rarity is %s', (rarity) => {
    it('should mark the card with it', () => {
      render(<WearableCard name="Hat" rarity={rarity} />)
      expect(card('Hat')).toHaveAttribute('data-rarity', rarity)
    })

    it("should take that rarity's foundation colour", () => {
      expect(tone(`.tone[data-rarity='${rarity}']`)).toBe(`--dcl-rarities-${rarity}`)
      expect(colours).toMatch(new RegExp(`--dcl-rarities-${rarity}: #[0-9a-f]{6};`))
    })
  })

  describe('when the card has no rarity, or one the foundations do not colour', () => {
    it('should be marked as base', () => {
      render(<WearableCard name="Plain shirt" />)
      expect(card('Plain shirt')).toHaveAttribute('data-rarity', 'base')
    })

    it('should take Neutrals/Silver, declared before every rarity so a rarity wins over it', () => {
      expect(tone('.tone')).toBe('--dcl-neutrals-silver')
      const base = tones.indexOf('.tone {')
      for (const rarity of RARITIES)
        expect(tones.indexOf(`.tone[data-rarity='${rarity}']`)).toBeGreaterThan(base)
    })

    it('should define no tone for a value that is not a rarity', () => {
      expect(() => tone(".tone[data-rarity='golden']")).toThrow('no rule')
      expect(() => tone(".tone[data-rarity='base']")).toThrow('no rule')
      render(<WearableCard name="Odd" rarity={'golden' as WearableCardRarity} />)
      expect(card('Odd')).toHaveAttribute('data-rarity', 'golden')
    })
  })

  describe("when the host passes its art", () => {
    it('should hand every picture to the stylesheet as a custom property, the tile included', () => {
      render(
        <WearableCard
          name="Hat"
          art={{ tile: '/tile.webp', flap: '/flap.webp', ring: '/ring.webp', hoverPanel: '/panel.webp' }}
        />
      )
      const root = card('Hat')
      expect(root.style.getPropertyValue('--wearable-card-tile')).toBe('url("/tile.webp")')
      expect(root.style.getPropertyValue('--wearable-card-flap')).toBe('url("/flap.webp")')
      expect(root.style.getPropertyValue('--wearable-card-ring')).toBe('url("/ring.webp")')
      expect(root.style.getPropertyValue('--wearable-card-hover-panel')).toBe('url("/panel.webp")')
      for (const span of root.querySelectorAll<HTMLElement>('span')) expect(span.getAttribute('style')).toBeNull()
      expect(root.querySelector('.body')).not.toHaveClass('plainTile')
    })

    it('should draw the tile from the picture it is handed', () => {
      expect(rule('.body')).toContain('background-image: var(--wearable-card-tile);')
    })

    it('should quote an inlined picture whole', () => {
      const data = 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg"/>'
      render(<WearableCard name="Hat" art={{ flap: data }} />)
      expect(card('Hat').style.getPropertyValue('--wearable-card-flap')).toContain(data)
    })
  })

  describe('when the host passes no art', () => {
    it('should set no picture at all', () => {
      render(<WearableCard name="Hat" />)
      const root = card('Hat')
      expect(root.getAttribute('style') ?? '').toBe('')
      for (const span of root.querySelectorAll<HTMLElement>('span'))
        expect(span.style.backgroundImage).toBe('')
      expect(root.querySelector('.body')).toHaveClass('plainTile')
    })

    it('should set only the pictures it was given', () => {
      render(<WearableCard name="Hat" art={{ ring: '/ring.webp' }} />)
      const root = card('Hat')
      expect(root.style.getPropertyValue('--wearable-card-ring')).toBe('url("/ring.webp")')
      expect(root.style.getPropertyValue('--wearable-card-tile')).toBe('')
      expect(root.style.getPropertyValue('--wearable-card-flap')).toBe('')
      expect(root.style.getPropertyValue('--wearable-card-hover-panel')).toBe('')
    })
  })

  describe('when the thumbnail is missing or fails', () => {
    it('should draw no image without one', () => {
      render(<WearableCard name="Hat" />)
      expect(card('Hat').querySelector('img')).toBeNull()
    })

    it('should draw no image for an empty one', () => {
      render(<WearableCard name="Hat" thumbnail="" />)
      expect(card('Hat').querySelector('img')).toBeNull()
    })

    it('should drop an image that fails to load, and keep the card', () => {
      render(<WearableCard name="Hat" thumbnail="/gone.png" />)
      const image = card('Hat').querySelector('img')
      expect(image).toHaveAttribute('src', '/gone.png')
      expect(image).toHaveAttribute('alt', '')
      fireEvent.error(image!)
      expect(card('Hat').querySelector('img')).toBeNull()
      expect(card('Hat')).toHaveTextContent('EQUIP')
    })

    it('should try a new thumbnail after one failed', () => {
      const view = render(<WearableCard name="Hat" thumbnail="/gone.png" />)
      fireEvent.error(card('Hat').querySelector('img')!)
      expect(card('Hat').querySelector('img')).toBeNull()
      view.rerender(<WearableCard name="Hat" thumbnail="/new.png" />)
      expect(card('Hat').querySelector('img')).toHaveAttribute('src', '/new.png')
    })
  })

  describe('when the name is long, or missing', () => {
    it('should carry the whole name as its accessible name and title', () => {
      const name = 'A very long wearable name that no tile of 126 pixels could ever show in one line'
      render(<WearableCard name={name} />)
      expect(card(name)).toHaveAttribute('title', name)
    })

    it('should render without a name', () => {
      render(<WearableCard />)
      const root = screen.getByRole('button', { pressed: false })
      expect(root).not.toHaveAttribute('aria-label')
      expect(root).not.toHaveAttribute('title')
    })
  })

  describe('when it decides the action it offers', () => {
    it('should offer EQUIP for an item that is not equipped', () => {
      render(<WearableCard name="Hat" />)
      expect(screen.getByText('EQUIP')).toHaveAttribute('role', 'button')
      expect(screen.queryByText('UNEQUIP')).toBeNull()
    })

    it('should offer UNEQUIP for an equipped item', () => {
      render(<WearableCard name="Hat" equipped />)
      expect(screen.getByText('UNEQUIP')).toBeInTheDocument()
      expect(screen.queryByText('EQUIP')).toBeNull()
    })

    it('should offer nothing for an equipped item that cannot be taken off', () => {
      render(<WearableCard name="Eyes" equipped unequippable={false} />)
      expect(card('Eyes')).toHaveTextContent('')
    })

    it('should still offer EQUIP for an item that cannot be taken off and is not equipped', () => {
      render(<WearableCard name="Eyes" unequippable={false} />)
      expect(screen.getByText('EQUIP')).toBeInTheDocument()
    })

    it('should offer nothing for an incompatible item, and say why', () => {
      render(<WearableCard name="Beard" incompatible />)
      expect(screen.queryByText('EQUIP')).toBeNull()
      expect(screen.getByText('Incompatible with body shape')).toBeInTheDocument()
    })

    it('should let an incompatible item that is equipped be taken off', () => {
      render(<WearableCard name="Beard" incompatible equipped />)
      expect(screen.getByText('UNEQUIP')).toBeInTheDocument()
    })

    it('should offer nothing on a tile to pick, equipped or not', () => {
      const view = render(<WearableCard name="Hat" pick />)
      expect(card('Hat')).toHaveTextContent('')
      view.rerender(<WearableCard name="Hat" pick equipped />)
      expect(card('Hat')).toHaveTextContent('')
    })
  })

  describe('when it shows its marks', () => {
    it('should show none by default', () => {
      render(<WearableCard name="Hat" pick />)
      expect(card('Hat')).toHaveTextContent('')
    })

    it('should show the new and smart marks, the slot number and the glyph', () => {
      render(
        <WearableCard
          name="Hat"
          pick
          isNew
          isSmart
          slotNumber={0}
          categoryIcon={<svg data-testid="glyph" />}
        />
      )
      expect(screen.getByText('NEW')).toBeInTheDocument()
      expect(screen.getByText('SMART')).toBeInTheDocument()
      expect(screen.getByText('0')).toBeInTheDocument()
      expect(screen.getByTestId('glyph')).toBeInTheDocument()
    })

    it('should show the count only above one', () => {
      const view = render(<WearableCard name="Hat" pick count={3} />)
      expect(screen.getByText('x3')).toBeInTheDocument()
      view.rerender(<WearableCard name="Hat" pick count={1} />)
      expect(card('Hat')).toHaveTextContent('')
      view.rerender(<WearableCard name="Hat" pick count={0} />)
      expect(card('Hat')).toHaveTextContent('')
    })

    it("should take the caller's texts", () => {
      const labels = {
        equip: 'PONER',
        unequip: 'QUITAR',
        isNew: 'NUEVO',
        smart: 'INTELIGENTE',
        incompatible: 'No sirve para este cuerpo'
      }
      const view = render(<WearableCard name="Hat" isNew isSmart labels={labels} />)
      expect(card('Hat')).toHaveTextContent('NUEVOINTELIGENTEPONER')
      view.rerender(<WearableCard name="Hat" equipped incompatible labels={labels} />)
      expect(card('Hat')).toHaveTextContent('No sirve para este cuerpoQUITAR')
    })

    it('should keep the default of a text the caller leaves out', () => {
      render(<WearableCard name="Hat" isNew labels={{ equip: 'WEAR' }} />)
      expect(card('Hat')).toHaveTextContent('NEWWEAR')
    })
  })

  describe('when it is selected', () => {
    it('should report its pressed state', () => {
      const view = render(<WearableCard name="Hat" />)
      expect(card('Hat')).toHaveAttribute('aria-pressed', 'false')
      view.rerender(<WearableCard name="Hat" selected />)
      expect(card('Hat')).toHaveAttribute('aria-pressed', 'true')
    })
  })

  describe('when it is pressed', () => {
    it('should call onClick for a press on the card', async () => {
      const onClick = vi.fn()
      const onEquip = vi.fn()
      render(<WearableCard name="Hat" onClick={onClick} onEquip={onEquip} />)
      await userEvent.click(card('Hat'))
      expect(onClick).toHaveBeenCalledTimes(1)
      expect(onEquip).not.toHaveBeenCalled()
    })

    it('should call onEquip alone for a press on the action', async () => {
      const onClick = vi.fn()
      const onEquip = vi.fn()
      render(<WearableCard name="Hat" onClick={onClick} onEquip={onEquip} />)
      await userEvent.click(screen.getByText('EQUIP'))
      expect(onEquip).toHaveBeenCalledTimes(1)
      expect(onClick).not.toHaveBeenCalled()
    })

    it('should press the action without a handler', async () => {
      const onClick = vi.fn()
      render(<WearableCard name="Hat" onClick={onClick} />)
      await userEvent.click(screen.getByText('EQUIP'))
      expect(onClick).not.toHaveBeenCalled()
    })

    it("should keep a double press on the action from the card's own handlers", async () => {
      const onClick = vi.fn()
      const onDoubleClick = vi.fn()
      const onEquip = vi.fn()
      render(
        <WearableCard name="Hat" onClick={onClick} onDoubleClick={onDoubleClick} onEquip={onEquip} />
      )
      await userEvent.dblClick(screen.getByText('EQUIP'))
      expect(onEquip).toHaveBeenCalledTimes(2)
      expect(onClick).not.toHaveBeenCalled()
      expect(onDoubleClick).not.toHaveBeenCalled()
    })

    it('should call onDoubleClick for a double press', async () => {
      const onDoubleClick = vi.fn()
      render(<WearableCard name="Hat" onDoubleClick={onDoubleClick} />)
      await userEvent.dblClick(card('Hat'))
      expect(onDoubleClick).toHaveBeenCalledTimes(1)
    })

    it('should be reached with Tab and pressed with Enter and Space', async () => {
      const onClick = vi.fn()
      render(<WearableCard name="Hat" onClick={onClick} />)
      await userEvent.tab()
      expect(card('Hat')).toHaveFocus()
      await userEvent.keyboard('{Enter}')
      await userEvent.keyboard(' ')
      expect(onClick).toHaveBeenCalledTimes(2)
    })

    it('should keep the action out of the tab order', async () => {
      render(<WearableCard name="Hat" />)
      await userEvent.tab()
      await userEvent.tab()
      expect(screen.getByText('EQUIP')).not.toHaveFocus()
    })

    it('should render and take a press without any handler', async () => {
      render(<WearableCard name="Hat" />)
      await userEvent.click(card('Hat'))
      await userEvent.hover(card('Hat'))
      expect(card('Hat')).toBeInTheDocument()
    })
  })

  describe('when a note covers it', () => {
    it('should show the note, report itself disabled and ignore a press', async () => {
      const onClick = vi.fn()
      render(<WearableCard name="Hat" pick coverNote="Equipped" onClick={onClick} />)
      expect(card('Hat')).toHaveAttribute('aria-disabled', 'true')
      expect(card('Hat')).toHaveTextContent('Equipped')
      await userEvent.click(card('Hat'))
      await userEvent.tab()
      await userEvent.keyboard('{Enter}')
      expect(onClick).not.toHaveBeenCalled()
    })

    it('should offer no action and ignore a double press on a tile that is not to pick', async () => {
      const onDoubleClick = vi.fn()
      const onEquip = vi.fn()
      render(
        <WearableCard name="Hat" coverNote="Equipped" onDoubleClick={onDoubleClick} onEquip={onEquip} />
      )
      expect(screen.queryByText('EQUIP')).toBeNull()
      await userEvent.dblClick(card('Hat'))
      expect(onDoubleClick).not.toHaveBeenCalled()
      expect(onEquip).not.toHaveBeenCalled()
    })

    it('should cover it with an empty note too', async () => {
      const onClick = vi.fn()
      render(<WearableCard name="Hat" pick coverNote="" onClick={onClick} />)
      expect(card('Hat')).toHaveAttribute('aria-disabled', 'true')
      await userEvent.click(card('Hat'))
      expect(onClick).not.toHaveBeenCalled()
    })

    it('should not report itself disabled without one', () => {
      render(<WearableCard name="Hat" pick />)
      expect(card('Hat')).not.toHaveAttribute('aria-disabled')
    })
  })

  describe('when the pointer enters and leaves', () => {
    it('should report each', async () => {
      const onHoverChange = vi.fn()
      render(<WearableCard name="Hat" onHoverChange={onHoverChange} />)
      await userEvent.hover(card('Hat'))
      expect(onHoverChange).toHaveBeenLastCalledWith(true)
      await userEvent.unhover(card('Hat'))
      expect(onHoverChange).toHaveBeenLastCalledWith(false)
      expect(onHoverChange).toHaveBeenCalledTimes(2)
    })
  })
})
