import { describe, it, expect } from 'vitest'
import { ItemType, type Item } from './items'
import { ItemTypeFilter, countItemsByType, filterItemsByType, paginateItems, parseItemTypeFilter } from './itemFilters'

const item = (id: string, type: ItemType) => ({ id, type }) as Item

const items = [
  item('w1', ItemType.WEARABLE),
  item('e1', ItemType.EMOTE),
  item('w2', ItemType.WEARABLE),
  item('e2', ItemType.EMOTE),
  item('w3', ItemType.WEARABLE)
]

describe('parseItemTypeFilter', () => {
  it('accepts the known filters and falls back to all', () => {
    expect(parseItemTypeFilter('emote')).toBe(ItemTypeFilter.EMOTE)
    expect(parseItemTypeFilter('wearable')).toBe(ItemTypeFilter.WEARABLE)
    expect(parseItemTypeFilter('bogus')).toBe(ItemTypeFilter.ALL)
    expect(parseItemTypeFilter(null)).toBe(ItemTypeFilter.ALL)
  })
})

describe('countItemsByType', () => {
  it('counts every type, reporting zero for missing ones', () => {
    expect(countItemsByType(items)).toEqual({ all: 5, wearable: 3, emote: 2 })
    expect(countItemsByType([item('w1', ItemType.WEARABLE)])).toEqual({ all: 1, wearable: 1, emote: 0 })
    expect(countItemsByType([])).toEqual({ all: 0, wearable: 0, emote: 0 })
  })
})

describe('filterItemsByType', () => {
  it('keeps only the requested type and everything for all', () => {
    expect(filterItemsByType(items, ItemTypeFilter.EMOTE).map(i => i.id)).toEqual(['e1', 'e2'])
    expect(filterItemsByType(items, ItemTypeFilter.WEARABLE).map(i => i.id)).toEqual(['w1', 'w2', 'w3'])
    expect(filterItemsByType(items, ItemTypeFilter.ALL)).toBe(items)
  })
})

describe('paginateItems', () => {
  it('slices the requested page and reports total and page count', () => {
    expect(paginateItems(items, 1, 2)).toEqual({ results: items.slice(0, 2), total: 5, pages: 3 })
    expect(paginateItems(items, 3, 2)).toEqual({ results: items.slice(4), total: 5, pages: 3 })
  })

  it('returns no results for an empty list or a page past the end', () => {
    expect(paginateItems([], 1, 20)).toEqual({ results: [], total: 0, pages: 0 })
    expect(paginateItems(items, 9, 2).results).toEqual([])
  })
})
