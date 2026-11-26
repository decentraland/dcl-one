import { describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import { WearableCard, rarityTile, type Rarity } from '../design'
import flap from '../assets/backpack/corner-flap.webp'
import ring from '../assets/backpack/equipped-outline.webp'
import hoverPanel from '../assets/backpack/hover-bg.webp'

// The HUD's WearableCard is the shared one (library/ui/components/WearableCard, tested there) drawn with the
// HUD's art: the rarity's tile texture and the flap, outline and hover panel shapes.
const RARITIES: Rarity[] = ['base', 'common', 'uncommon', 'rare', 'epic', 'legendary', 'mythic', 'unique', 'exotic']

const tileOf = (card: HTMLElement): string => card.style.getPropertyValue('--wearable-card-tile')

describe('WearableCard', () => {
  it.each(RARITIES)('draws a %s card on that rarity\'s texture', (rarity) => {
    render(<WearableCard name="Hat" rarity={rarity} />)
    const card = screen.getByRole('button', { name: 'Hat' })
    expect(card.getAttribute('data-rarity')).toBe(rarity)
    expect(tileOf(card)).toBe(`url("${rarityTile(rarity)}")`)
  })

  it('draws each rarity on a texture of its own', () => {
    expect(new Set(RARITIES.map((rarity) => rarityTile(rarity))).size).toBe(RARITIES.length)
  })

  it('draws a card without a rarity, or with one it does not know, on the base texture', () => {
    const view = render(<WearableCard name="Hat" />)
    expect(tileOf(screen.getByRole('button', { name: 'Hat' }))).toBe(`url("${rarityTile('base')}")`)
    view.rerender(<WearableCard name="Hat" rarity={'golden' as Rarity} />)
    expect(tileOf(screen.getByRole('button', { name: 'Hat' }))).toBe(`url("${rarityTile('base')}")`)
  })

  it('passes the HUD flap, equipped outline and hover panel', () => {
    render(<WearableCard name="Hat" equipped />)
    const card = screen.getByRole('button', { name: 'Hat' })
    expect(card.style.getPropertyValue('--wearable-card-flap')).toBe(`url("${flap}")`)
    expect(card.style.getPropertyValue('--wearable-card-ring')).toBe(`url("${ring}")`)
    expect(card.style.getPropertyValue('--wearable-card-hover-panel')).toBe(`url("${hoverPanel}")`)
  })

  it('keeps the props its call sites pass', () => {
    const onClick = vi.fn()
    const onEquip = vi.fn()
    render(<WearableCard name="Hat" equipped selected isNew isSmart slotNumber={3} onClick={onClick} onEquip={onEquip} />)
    const card = screen.getByRole('button', { name: 'Hat' })
    expect(card.getAttribute('aria-pressed')).toBe('true')
    expect(card.textContent).toBe('NEWSMART3UNEQUIP')
    act(() => screen.getByText('UNEQUIP').click())
    expect(onEquip).toHaveBeenCalledTimes(1)
    expect(onClick).not.toHaveBeenCalled()
    act(() => card.click())
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('a covered tile to pick takes no press', () => {
    const onClick = vi.fn()
    render(<WearableCard pick name="Hat" coverNote="Equipped" count={2} onClick={onClick} />)
    const card = screen.getByRole('button', { name: 'Hat' })
    expect(card.getAttribute('aria-disabled')).toBe('true')
    expect(card.textContent).toBe('x2Equipped')
    act(() => card.click())
    expect(onClick).not.toHaveBeenCalled()
  })
})
