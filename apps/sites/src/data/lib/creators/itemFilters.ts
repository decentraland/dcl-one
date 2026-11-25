import { ItemType, type Item } from './items'

export enum ItemTypeFilter {
  ALL = 'all',
  WEARABLE = 'wearable',
  EMOTE = 'emote'
}

export const ITEM_TYPE_FILTERS = [ItemTypeFilter.ALL, ItemTypeFilter.WEARABLE, ItemTypeFilter.EMOTE] as const

export function parseItemTypeFilter(raw: string | null): ItemTypeFilter {
  return ITEM_TYPE_FILTERS.includes(raw as ItemTypeFilter) ? (raw as ItemTypeFilter) : ItemTypeFilter.ALL
}

export function countItemsByType(items: Item[]): Record<ItemTypeFilter, number> {
  const counts = { [ItemTypeFilter.ALL]: items.length, [ItemTypeFilter.WEARABLE]: 0, [ItemTypeFilter.EMOTE]: 0 }
  for (const item of items) {
    if (item.type === ItemType.EMOTE) counts[ItemTypeFilter.EMOTE] += 1
    else counts[ItemTypeFilter.WEARABLE] += 1
  }
  return counts
}

export function filterItemsByType(items: Item[], filter: ItemTypeFilter): Item[] {
  if (filter === ItemTypeFilter.ALL) return items
  const type = filter === ItemTypeFilter.EMOTE ? ItemType.EMOTE : ItemType.WEARABLE
  return items.filter(item => item.type === type)
}

/** Client-side page over an in-memory list: builder-server can't filter items by type, so paging happens here. */
export function paginateItems<T>(
  items: T[],
  page: number,
  limit: number
): { results: T[]; total: number; pages: number } {
  const total = items.length
  const pages = Math.ceil(total / limit)
  const start = (page - 1) * limit
  return { results: items.slice(start, start + limit), total, pages }
}
