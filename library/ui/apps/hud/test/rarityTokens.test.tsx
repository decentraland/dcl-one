import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, it, expect } from 'vitest'
import { render } from '@testing-library/react'
import { WearableCard } from '../design/WearableCard'
import { EmoteSlot } from '../features/emotes/EmoteSlot'

// The Backpack's cards (--rm / --rl) and the Shop's cards (--rarity / --rarity-light) draw a rarity with
// the same palette colour. jsdom resolves no custom property through a stylesheet, so this reads the rules.
const RARITIES = ['common', 'uncommon', 'rare', 'epic', 'legendary', 'mythic', 'unique', 'exotic'] as const

const tokens = readFileSync(join(__dirname, '../styles/tokens.css'), 'utf8')
const foundations = readFileSync(join(__dirname, '../../../foundations/colors/colors.css'), 'utf8')

function declarations(selector: string): Record<string, string> {
  const rule = [...tokens.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(([, sel]) => sel.replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector)
  if (!rule) throw new Error(`no rule ${selector}`)
  return Object.fromEntries([...rule[2].matchAll(/(--[\w-]+):\s*([^;]+);/g)].map(([, name, value]) => [name, value.trim()]))
}

function foundationColour(value: string): string {
  const name = /^var\((--dcl-[\w-]+)\)$/.exec(value)?.[1]
  if (!name) throw new Error(`${value} is not a foundation token`)
  const hex = new RegExp(`${name}:\\s*(#[0-9a-f]{6});`).exec(foundations)?.[1]
  if (!hex) throw new Error(`${name} has no colour in the foundations`)
  return hex
}

describe('rarity colours', () => {
  describe.each(RARITIES)('when the rarity is %s', (rarity) => {
    const rule = declarations(`[data-rarity='${rarity}']`)

    it('should draw the Backpack main tone with the palette colour of that rarity', () => {
      expect(rule['--rm']).toBe(`var(--dcl-rarities-${rarity})`)
      expect(foundationColour(rule['--rm'])).toBe(foundationColour(rule['--rarity']))
    })

    it('should draw the Backpack light tone with the palette light colour of that rarity', () => {
      expect(rule['--rl']).toBe(`var(--dcl-rarities-${rarity}-light)`)
      expect(foundationColour(rule['--rl'])).toBe(foundationColour(rule['--rarity-light']))
    })

    it("should draw the emote slot's flap with the palette colour of that rarity", () => {
      const { container } = render(<EmoteSlot rarity={rarity} />)
      expect(container.innerHTML.toLowerCase()).toContain(`fill="${foundationColour(rule['--rm'])}"`)
    })
  })

  describe('when the rarity is unknown or missing', () => {
    it('should fall through to the neutral base tones', () => {
      const base = declarations('[data-rarity]')
      expect(base['--rm']).toBe('var(--dcl-neutrals-silver)')
      expect(base['--rl']).toBe('var(--dcl-neutrals-gray)')
    })

    it('should declare the base before every rarity, so a known rarity wins over it', () => {
      const baseAt = tokens.indexOf('[data-rarity] {')
      expect(baseAt).toBeGreaterThan(-1)
      for (const rarity of RARITIES) expect(tokens.indexOf(`[data-rarity='${rarity}'] {`)).toBeGreaterThan(baseAt)
    })

    it('should define no tones for a value that is not a rarity', () => {
      expect(() => declarations("[data-rarity='golden']")).toThrow('no rule')
    })

    it('should mark a Backpack card without a rarity as base', () => {
      const { getByRole } = render(<WearableCard name="Plain shirt" />)
      expect(getByRole('button', { name: 'Plain shirt' }).getAttribute('data-rarity')).toBe('base')
    })
  })

  it('should mark a mythic Backpack card with its rarity', () => {
    const { getByRole } = render(<WearableCard name="Mythic crown" rarity="mythic" />)
    expect(getByRole('button', { name: 'Mythic crown' }).getAttribute('data-rarity')).toBe('mythic')
  })
})
