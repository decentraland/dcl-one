// The Backpack's randomize outfit: one random wearable per category from the base set and the pages
// of the player's own wearables fetched for the roll, with picks that hide each other resolved.
// Shared with the bridge scene (like avatarHides.ts); pure, with the random source passed in, so
// vitest covers it. Mirrors Unity's OutfitRandomizer.

import { isCompatible } from './bodyShape'

/** What a roll needs to know about a wearable (a catalog `Wearable` fits). */
export interface RollItem {
  urn: string
  category: string
  bodyShapes?: string[]
  isSmart?: boolean
  /** Categories it hides when worn on the roll's body shape. */
  hides?: string[]
}

/** Chance that an optional accessory category (hat, mask, earring, facial hair, …) gets a pick on a roll. */
export const OPTIONAL_INCLUDE_CHANCE = 0.75

/** Chance that a roll includes a skin, kept low because a skin covers most of the outfit. */
export const SKIN_INCLUDE_CHANCE = 0.2

/** Chance that a category picks from the player's own wearables before falling back to the base ones. */
export const CUSTOM_PRIORITY_CHANCE = 0.8

/** How many times a roll retries the core categories a conflict left bare before giving up on them. */
export const MAX_REPICK_ROUNDS = 3

/** Every category a roll can fill, in hiding priority order: an earlier category wins a conflict. */
export const OUTFIT_CATEGORIES = [
  'skin', 'upper_body', 'hands_wear', 'lower_body', 'feet', 'helmet', 'hat', 'top_head', 'mask', 'eyewear',
  'earring', 'tiara', 'hair', 'eyebrows', 'eyes', 'mouth', 'facial_hair'
]

const OPTIONAL_CATEGORIES = new Set(['facial_hair', 'hat', 'mask', 'tiara', 'helmet', 'earring', 'eyewear', 'top_head', 'hands_wear'])
const FEMALE_EXCLUDED_CATEGORIES = new Set(['facial_hair'])
const FEMALE_BODY_SHAPE = /:base-avatars:basefemale$/i

/** Core categories are the ones a roll always tries to fill. */
export function isCoreCategory(category: string): boolean {
  return category !== 'skin' && !OPTIONAL_CATEGORIES.has(category)
}

export function isBaseWearable(urn: string): boolean {
  return urn.includes(':base-avatars:')
}

function includeChance(category: string): number {
  if (category === 'skin') return SKIN_INCLUDE_CHANCE
  return OPTIONAL_CATEGORIES.has(category) ? OPTIONAL_INCLUDE_CHANCE : 1
}

interface CategoryPool {
  custom: RollItem[]
  base: RollItem[]
  /** Index in `custom` of the wearable its page was fetched for, or -1. */
  preferred: number
}

/**
 * Walks the picks in hiding priority order and keeps a pick only when no kept pick hides it and it
 * hides no kept pick. `hidden` holds every category the kept picks hide.
 */
export function resolveConflicts(picks: ReadonlyMap<string, RollItem>): { dropped: string[]; hidden: Set<string> } {
  const kept = new Set<string>()
  const hidden = new Set<string>()
  const dropped: string[] = []
  for (const category of OUTFIT_CATEGORIES) {
    const item = picks.get(category)
    if (item == null) continue
    const hides = item.hides ?? []
    if (hidden.has(category) || hides.some((h) => kept.has(h))) {
      dropped.push(category)
      continue
    }
    kept.add(category)
    for (const h of hides) hidden.add(h)
  }
  return { dropped, hidden }
}

export class OutfitRandomizer {
  private readonly pools = new Map<string, CategoryPool>()
  private bodyShape = ''

  constructor(private readonly random: () => number = Math.random) {}

  private get isFemale(): boolean {
    return FEMALE_BODY_SHAPE.test(this.bodyShape)
  }

  /**
   * Replaces the catalog with the base wearables that fit `bodyShape`, dropping the owned pages of the
   * previous roll. Body shapes, smart wearables and wearables without a representation for the body
   * shape are left out, here and in `addOwnedPage`.
   */
  buildCatalog(baseItems: readonly RollItem[], bodyShape: string): void {
    this.pools.clear()
    this.bodyShape = bodyShape
    for (const item of baseItems) if (isBaseWearable(item.urn) && this.isUsable(item)) this.pool(item.category).base.push(item)
  }

  /**
   * The categories this roll fills: every core category and, by their odds, the optional ones and skin.
   * A category is a candidate when the catalog has wearables for it or `ownedTotals` reports owned ones.
   */
  chooseCategories(ownedTotals: ReadonlyMap<string, number>): string[] {
    const chosen: string[] = []
    for (const category of OUTFIT_CATEGORIES) {
      if (this.isFemale && FEMALE_EXCLUDED_CATEGORIES.has(category)) continue
      const pool = this.pools.get(category)
      const hasEntries = pool != null && pool.custom.length + pool.base.length > 0
      if (!hasEntries && (ownedTotals.get(category) ?? 0) === 0) continue
      if (this.random() >= includeChance(category)) continue
      chosen.push(category)
    }
    return chosen
  }

  /** A uniform random index in [0, count). */
  pickIndex(count: number): number {
    return Math.min(count - 1, Math.floor(this.random() * count))
  }

  /**
   * Adds the usable wearables of `category` from a fetched page of owned wearables. The one at
   * `preferredIndex`, when usable, is the category's first pick whenever custom is preferred.
   */
  addOwnedPage(category: string, page: readonly RollItem[], preferredIndex: number): void {
    page.forEach((item, i) => {
      // The base wearables are already in the catalog
      if (item.category !== category || isBaseWearable(item.urn) || !this.isUsable(item)) return
      const pool = this.pool(category)
      if (i === preferredIndex) pool.preferred = pool.custom.length
      pool.custom.push(item)
    })
  }

  /**
   * One pick per chosen category, conflicts resolved. A core category left bare because its pick hid a
   * kept one gets another untried wearable, up to MAX_REPICK_ROUNDS times; one hidden by a kept pick is
   * covered on the avatar, so it stays empty.
   */
  roll(chosen: readonly string[]): RollItem[] {
    const picks = new Map<string, RollItem>()
    for (const category of chosen) {
      const pool = this.pools.get(category)
      const item = pool != null ? this.pickFrom(pool, null) : undefined
      if (item != null) picks.set(category, item)
    }
    const tried = new Set([...picks.values()].map((item) => item.urn))
    for (let round = 0; round <= MAX_REPICK_ROUNDS; round++) {
      const { dropped, hidden } = resolveConflicts(picks)
      let repicked = false
      for (const category of dropped) {
        picks.delete(category)
        const pool = this.pools.get(category)
        if (round === MAX_REPICK_ROUNDS || pool == null || !isCoreCategory(category) || hidden.has(category)) continue
        const item = this.pickFrom(pool, tried)
        if (item == null) continue
        picks.set(category, item)
        tried.add(item.urn)
        repicked = true
      }
      if (!repicked) break
    }
    return [...picks.values()]
  }

  private pool(category: string): CategoryPool {
    let pool = this.pools.get(category)
    if (pool == null) {
      pool = { custom: [], base: [], preferred: -1 }
      this.pools.set(category, pool)
    }
    return pool
  }

  private isUsable(item: RollItem): boolean {
    if (item.category === 'body_shape' || item.isSmart === true) return false
    if (this.isFemale && FEMALE_EXCLUDED_CATEGORIES.has(item.category)) return false
    return isCompatible(item, this.bodyShape || undefined)
  }

  /**
   * Tries the preferred pool first and falls back to the other one, so a category with a single kind of
   * wearable always yields a pick. A first pick that prefers custom takes the wearable its page was
   * fetched for, so every owned wearable is equally likely.
   */
  private pickFrom(pool: CategoryPool, tried: ReadonlySet<string> | null): RollItem | undefined {
    const preferCustom = this.random() < CUSTOM_PRIORITY_CHANCE
    if (preferCustom && tried == null && pool.preferred >= 0) return pool.custom[pool.preferred]
    const [preferred, fallback] = preferCustom ? [pool.custom, pool.base] : [pool.base, pool.custom]
    return this.pickUntried(preferred, tried) ?? this.pickUntried(fallback, tried)
  }

  private pickUntried(items: readonly RollItem[], tried: ReadonlySet<string> | null): RollItem | undefined {
    const remaining = tried == null ? items : items.filter((item) => !tried.has(item.urn))
    return remaining.length > 0 ? remaining[this.pickIndex(remaining.length)] : undefined
  }
}
