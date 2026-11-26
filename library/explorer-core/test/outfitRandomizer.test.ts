import { describe, it, expect } from 'vitest'
import { OUTFIT_CATEGORIES, OutfitRandomizer, resolveConflicts, type RollItem } from '../engine/outfitRandomizer'

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

const base = (name: string, category: string, extra: Partial<RollItem> = {}): RollItem => ({
  urn: `urn:decentraland:off-chain:base-avatars:${name}`,
  category,
  bodyShapes: [MALE, FEMALE],
  ...extra
})
const owned = (id: number, category: string, extra: Partial<RollItem> = {}): RollItem => ({
  urn: `urn:decentraland:matic:collections-v2:0xabc:${id}`,
  category,
  bodyShapes: [MALE, FEMALE],
  ...extra
})

// A random source that always returns `value`: 0 includes every category, prefers owned wearables and
// takes the first index; 0.9 skips optional categories and skin, prefers base and takes a late index.
const always = (value: number) => () => value

const NO_OWNED = new Map<string, number>()
const CORE_BASE = [base('tshirt', 'upper_body'), base('jeans', 'lower_body'), base('sneakers', 'feet'), base('hair', 'hair')]

function roll(randomizer: OutfitRandomizer, totals: ReadonlyMap<string, number> = NO_OWNED): Map<string, string> {
  const outfit = randomizer.roll(randomizer.chooseCategories(totals))
  return new Map(outfit.map((item) => [item.category, item.urn]))
}

describe('outfit randomizer', () => {
  it('fills every core category and skips the optional ones and skin when their odds fail', () => {
    const r = new OutfitRandomizer(always(0.9))
    r.buildCatalog([...CORE_BASE, base('cap', 'hat'), base('skin', 'skin')], MALE)
    expect([...roll(r).keys()].sort()).toEqual(['feet', 'hair', 'lower_body', 'upper_body'])
  })

  it('includes the optional categories and skin when their odds pass', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog([base('cap', 'hat'), base('glasses', 'eyewear')], MALE)
    expect([...roll(r).keys()].sort()).toEqual(['eyewear', 'hat'])
  })

  it('picks the owned wearable its page was fetched for when owned is preferred', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog(CORE_BASE, MALE)
    r.addOwnedPage('upper_body', [owned(1, 'upper_body'), owned(2, 'upper_body'), owned(3, 'upper_body')], 2)
    expect(roll(r).get('upper_body')).toBe(owned(3, 'upper_body').urn)
  })

  it('picks a base wearable when base is preferred', () => {
    const r = new OutfitRandomizer(always(0.9))
    r.buildCatalog(CORE_BASE, MALE)
    r.addOwnedPage('upper_body', [owned(1, 'upper_body')], 0)
    expect(roll(r).get('upper_body')).toBe(base('tshirt', 'upper_body').urn)
  })

  it('keeps only the usable owned wearables of the page category', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog([], MALE)
    r.addOwnedPage(
      'upper_body',
      [
        owned(1, 'lower_body'),
        owned(2, 'upper_body', { isSmart: true }),
        owned(3, 'upper_body', { bodyShapes: [FEMALE] }),
        base('tshirt', 'upper_body'),
        owned(4, 'upper_body')
      ],
      0
    )
    expect(roll(r, new Map([['upper_body', 5]])).get('upper_body')).toBe(owned(4, 'upper_body').urn)
  })

  it('drops the previous roll pages on a new catalog', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog(CORE_BASE, MALE)
    r.addOwnedPage('upper_body', [owned(1, 'upper_body')], 0)
    r.buildCatalog(CORE_BASE, MALE)
    expect(roll(r).get('upper_body')).toBe(base('tshirt', 'upper_body').urn)
  })

  it('chooses a category that only has owned wearables', () => {
    const r = new OutfitRandomizer(always(0.9))
    r.buildCatalog([], MALE)
    expect(r.chooseCategories(new Map([['upper_body', 3]]))).toEqual(['upper_body'])
  })

  it('leaves out facial hair and wearables without a representation for the body shape', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog([base('beard', 'facial_hair'), base('male_only', 'upper_body', { bodyShapes: [MALE] }), base('dress', 'upper_body')], FEMALE)
    const picks = roll(r, new Map([['facial_hair', 2]]))
    expect(picks.has('facial_hair')).toBe(false)
    expect(picks.get('upper_body')).toBe(base('dress', 'upper_body').urn)
  })

  it('never picks body shapes or smart base wearables', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog([base('BaseMale', 'body_shape'), base('smart', 'hat', { isSmart: true })], MALE)
    expect(roll(r).size).toBe(0)
  })

  it('maps a random index into [0, count)', () => {
    expect(new OutfitRandomizer(always(0.999999)).pickIndex(45)).toBe(44)
    expect(new OutfitRandomizer(always(0)).pickIndex(45)).toBe(0)
  })
})

describe('outfit conflicts', () => {
  it('lets the earlier category win: a skin hides the clothes under it', () => {
    const picks = new Map([
      ['skin', base('skin', 'skin', { hides: ['upper_body', 'lower_body'] })],
      ['upper_body', base('tshirt', 'upper_body')]
    ])
    const { dropped, hidden } = resolveConflicts(picks)
    expect(dropped).toEqual(['upper_body'])
    expect(hidden.has('upper_body')).toBe(true)
  })

  it('drops a later pick that hides a kept one', () => {
    const picks = new Map([
      ['hat', base('cap', 'hat')],
      ['hair', base('long', 'hair', { hides: ['hat'] })]
    ])
    expect(resolveConflicts(picks).dropped).toEqual(['hair'])
  })

  it('repicks a core category left bare by a conflict', () => {
    // 0.4 takes the first hair, which hides the hat; the repick can only take the other one
    const r = new OutfitRandomizer(always(0.4))
    r.buildCatalog([base('cap', 'hat'), base('long', 'hair', { hides: ['hat'] }), base('short', 'hair')], MALE)
    const outfit = r.roll(['hat', 'hair'])
    expect(outfit.map((i) => i.urn).sort()).toEqual([base('cap', 'hat').urn, base('short', 'hair').urn].sort())
  })

  it('leaves a core category a kept pick covers empty', () => {
    const r = new OutfitRandomizer(always(0))
    r.buildCatalog([base('skin', 'skin', { hides: ['upper_body'] }), base('tshirt', 'upper_body'), base('hoodie', 'upper_body')], MALE)
    expect(r.roll(['skin', 'upper_body']).map((i) => i.category)).toEqual(['skin'])
  })

  it('walks the categories in hiding priority order', () => {
    expect(OUTFIT_CATEGORIES[0]).toBe('skin')
    expect(OUTFIT_CATEGORIES).not.toContain('body_shape')
  })
})
