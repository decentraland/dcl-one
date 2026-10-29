// from: shop@94584a5c:app/src/lib/searchFacets.spec.ts
import { describe, it, expect } from 'vitest'
import {
  CATEGORY_ALIASES,
  FACETS,
  RARITY_ALIASES,
  facetId,
  facetUrl,
  facetsFor,
  normalizeAlias
} from './searchFacets'
import { CATEGORIES } from './categories'
import { RARITIES } from './rarity'
import { t } from '../intl'

const key = (query: string) => facetsFor(query).map(facetId)

describe('when normalizing an alias or a query', () => {
  it('should treat case, accents, surrounding and repeated spaces the same on both sides', () => {
    expect(normalizeAlias('  MÁSCARA  ')).toBe('mascara')
    expect(normalizeAlias('Máscara')).toBe('mascara')
    expect(normalizeAlias('upper   body')).toBe('upper body')
    for (const aliases of [...Object.values(CATEGORY_ALIASES), ...Object.values(RARITY_ALIASES)])
      for (const alias of aliases) expect(normalizeAlias(alias)).toBe(alias)
  })

  it('should cut no punctuation: the hyphenated and the plain spelling are two listed aliases', () => {
    expect(normalizeAlias('t-shirt')).toBe('t-shirt')
    expect(key('t-shirt')).toEqual(['category:Upper Body'])
    expect(key('tshirt')).toEqual(['category:Upper Body'])
    expect(key('t shirt')).toEqual([])
  })
})

describe('the facet dictionary', () => {
  const categoryKeys = new Set<string>()
  for (const top of CATEGORIES) {
    if (top.key === 'all' || top.key === 'names') continue
    categoryKeys.add(top.key)
    for (const sub of top.subs ?? []) {
      categoryKeys.add(sub.key)
      for (const leaf of sub.subs ?? []) categoryKeys.add(leaf.key)
    }
  }

  it('should name only keys the sidebar and the rarity filter know, and every one of them', () => {
    expect(new Set(Object.keys(CATEGORY_ALIASES))).toEqual(categoryKeys)
    expect(new Set(Object.keys(RARITY_ALIASES))).toEqual(new Set(RARITIES))
    expect(FACETS.map(facetId).sort()).toEqual(
      [...[...categoryKeys].map(k => `category:${k}`), ...RARITIES.map(k => `rarity:${k}`)].sort()
    )
  })

  it('should list the (English) label of every facet among its aliases', () => {
    for (const facet of FACETS) {
      const aliases = facet.kind === 'category' ? CATEGORY_ALIASES[facet.key] : RARITY_ALIASES[facet.key]
      const labelKey = facet.kind === 'category' ? facet.labelKey : `rarity.${facet.key}`
      const text = normalizeAlias(t(labelKey))
      expect(aliases, `${facetId(facet)} lacks "${text}"`).toContain(text)
    }
  })

  it('should give no alias to two facets', () => {
    const seen = new Map<string, string>()
    for (const facet of FACETS) {
      const aliases = facet.kind === 'category' ? CATEGORY_ALIASES[facet.key] : RARITY_ALIASES[facet.key]
      for (const alias of aliases ?? []) {
        expect(seen.get(alias), `"${alias}" is listed under ${seen.get(alias)} and ${facetId(facet)}`).toBeUndefined()
        seen.set(alias, facetId(facet))
      }
    }
  })
})

describe('when a query names a category or a rarity', () => {
  it('should recognise what the 90 days of searches actually said, and nothing else among them', () => {
    // 19 observed queries: 16 categories in, 3 out
    const observed: Record<string, string[]> = {
      top: ['category:Upper Body'],
      dance: ['category:Dance'],
      dress: ['category:Upper Body'],
      pants: ['category:Lower Body'],
      eyes: ['category:Eyes'],
      boots: ['category:Feet'],
      hair: ['category:Hair'],
      shirt: ['category:Upper Body'],
      shoes: ['category:Feet'],
      sneakers: ['category:Feet'],
      hoodie: ['category:Upper Body'],
      hat: ['category:Hat'],
      mask: ['category:Mask'],
      helmet: ['category:Helmet'],
      torso: ['category:Upper Body'],
      skirt: ['category:Lower Body'],
      wings: [],
      necklace: [],
      free: []
    }
    for (const [query, expected] of Object.entries(observed)) expect(key(query), query).toEqual(expected)
  })

  it('should match the whole query only, so a phrase that merely ends in a category word keeps its text results', () => {
    for (const negative of [
      'pirate hat',
      'hat pirate',
      'rare pepe',
      'red dragon hoodie',
      'top hat',
      'epic games',
      'legendary duck',
      'tophat',
      'hatsune',
      'dog emote'
    ])
      expect(key(negative), negative).toEqual([])
  })

  it('should stay quiet for the frequent searches that are not a category', () => {
    for (const negative of [
      'duck',
      'sword',
      'kimono',
      'doki',
      'rtfkt',
      'glow',
      'moon',
      'doll',
      'atari rtfkt',
      'coca cola',
      'the',
      'x',
      'sh',
      'p',
      '"p',
      'land',
      'art',
      'pink',
      'alien'
    ])
      expect(key(negative), negative).toEqual([])
  })

  it('should answer to multi-word aliases, to Spanish and to accents', () => {
    expect(key('upper body')).toEqual(['category:Upper Body'])
    expect(key('Top Head')).toEqual(['category:Top Head'])
    expect(key('facial hair')).toEqual(['category:Facial Hair'])
    expect(key('máscara')).toEqual(['category:Mask'])
    expect(key('mascara')).toEqual(['category:Mask'])
    expect(key('Sombreros')).toEqual(['category:Hat'])
    expect(key('zapatillas')).toEqual(['category:Feet'])
    expect(key('baile')).toEqual(['category:Dance'])
    expect(key('poco común')).toEqual(['rarity:uncommon'])
    expect(key('Épico')).toEqual(['rarity:epic'])
    expect(key('legendary')).toEqual(['rarity:legendary'])
  })

  it('should send "top" to Upper Body alone', () => {
    expect(key('top')).toEqual(['category:Upper Body'])
    expect(key('tops')).toEqual(['category:Upper Body'])
  })

  it('should offer the whole category for its own name, and the parents for theirs', () => {
    expect(key('wearables')).toEqual(['category:wearable'])
    expect(key('emotes')).toEqual(['category:emote'])
    expect(key('accessories')).toEqual(['category:Accessories'])
    expect(key('head')).toEqual(['category:Head'])
  })

  it('should ignore a single character', () => {
    expect(key('h')).toEqual([])
    expect(key(' ')).toEqual([])
  })
})

describe('the destination of a facet', () => {
  const facet = (id: string) => FACETS.find(f => facetId(f) === id)!

  it('should be the URL the sidebar would produce, from nothing else', () => {
    expect(facetUrl(facet('category:Hat'))).toBe('/shop/items?category=wearable&subCategory=Hat')
    expect(facetUrl(facet('category:Upper Body'))).toBe('/shop/items?category=wearable&subCategory=Upper+Body')
    expect(facetUrl(facet('category:Accessories'))).toBe('/shop/items?category=wearable&subCategory=Accessories')
    expect(facetUrl(facet('category:Dance'))).toBe('/shop/items?category=emote&subCategory=Dance')
    expect(facetUrl(facet('category:emote'))).toBe('/shop/items?category=emote')
    expect(facetUrl(facet('rarity:epic'))).toBe('/shop/items?rarities=epic')
  })

  it('should carry a key with spaces in a form the results page reads back', () => {
    const url = new URL(facetUrl(facet('category:Upper Body')), 'https://shop.example')
    expect(url.searchParams.get('subCategory')).toBe('Upper Body')
  })

  it('should describe a category with its icon and its parents, outermost first', () => {
    expect(facet('category:Hat')).toMatchObject({
      top: 'wearable',
      icon: 'cat-hat',
      parents: ['categories.wearables', 'categories.accessories']
    })
    expect(facet('category:Feet')).toMatchObject({ icon: 'cat-feet', parents: ['categories.wearables'] })
    expect(facet('category:wearable')).toMatchObject({ parents: [] })
  })
})
