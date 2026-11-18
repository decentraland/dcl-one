// Rarity constants shared by every rarity control (pill, select, forms).
import { rarities } from '@/data/lib/creators/theme'

export const RARITIES = ['unique', 'mythic', 'exotic', 'legendary', 'epic', 'rare', 'uncommon', 'common'] as const

export type RarityName = (typeof RARITIES)[number]

export const DEFAULT_RARITY: RarityName = 'epic'

/** Max supply per rarity (legacy Rarity.getMaxSupply). */
export const RARITY_MAX_SUPPLY: Record<RarityName, number> = {
  unique: 1,
  mythic: 10,
  exotic: 50,
  legendary: 100,
  epic: 1000,
  rare: 5000,
  uncommon: 10000,
  common: 100000
}

export function isRarity(value: string | null | undefined): value is RarityName {
  return !!value && (RARITIES as readonly string[]).includes(value)
}

/** The design palette color of a rarity; undefined for an unknown rarity. */
function getRarityColor(rarity: string | null | undefined): string | undefined {
  const key = rarity?.toLowerCase()
  return isRarity(key) ? rarities[key] : undefined
}

function parseHex(color: string): [number, number, number] {
  const hex = color.replace('#', '')
  return [parseInt(hex.slice(0, 2), 16), parseInt(hex.slice(2, 4), 16), parseInt(hex.slice(4, 6), 16)]
}

/**
 * Rarity wash behind an item's artwork (shop's card media): the rarity color, light at the center so the
 * artwork still cuts against it and gathering toward the edges. `undefined` for an unknown rarity, so the
 * media keeps its neutral fill.
 */
export function getRarityMediaBackground(rarity: string | null | undefined): string | undefined {
  const color = getRarityColor(rarity)
  return color ? mediaWash(color) : undefined
}

function mediaWash(color: string): string {
  const [r, g, b] = parseHex(color)
  const stop = (alpha: number) => `rgba(${r}, ${g}, ${b}, ${alpha})`
  return `radial-gradient(circle at 50% 38%, ${stop(0.04)} 0%, ${stop(0.3)} 50%, ${stop(0.62)} 100%)`
}

export function getRarityMaxSupply(rarity: string | null | undefined): number | undefined {
  return isRarity(rarity) ? RARITY_MAX_SUPPLY[rarity] : undefined
}

/** Compact supply for labels: 100 → "100", 1000 → "1K", 100000 → "100K". */
export function formatSupply(supply: number): string {
  if (supply >= 1000 && supply % 1000 === 0) return `${supply / 1000}K`
  return String(supply)
}

/** "Legendary (100)" / "Epic (1K)" — the rarity label every list and form uses. */
export function formatRarityLabel(label: string, rarity: string | null | undefined): string {
  const supply = getRarityMaxSupply(rarity)
  return supply === undefined ? label : `${label} (${formatSupply(supply)})`
}

/**
 * One entry of builder-server `GET /rarities`: the subgraph rarity plus `prices` — the USD price
 * from the graph and the MANA price the RaritiesWithOracle contract converts it to. Both are wei strings.
 */
export type BlockchainRarity = {
  id: string
  name: string
  price: string
  maxSupply: string
  prices?: {
    MANA: string
    USD: string
  }
}
