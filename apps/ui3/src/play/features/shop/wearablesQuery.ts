// The in-world Shop's wearables grid: the web Shop's catalogue fetcher (shop/discovery/api), its
// category map and sorts, behind one TanStack key and options factory, so every reader of one filter
// set shares one cache entry. The feed is the web Shop's default one: everything on sale, primary
// mints and resales, which the card prices by the web Shop's own rule (shop/discovery/cardPrice).

import { infiniteQueryOptions, keepPreviousData, useInfiniteQuery } from '@tanstack/react-query'
import { fetchShopItems, type ItemsPage, type ShopSort } from '../../../shop/discovery/api'
import { SUBCAT_MAP } from '../../../shop/discovery/lib/categories'
import allIcon from '../../assets/category-icons/all.png'
import hairIcon from '../../assets/category-icons/hair.png'
import upperBodyIcon from '../../assets/category-icons/upper_body.png'
import handsWearIcon from '../../assets/category-icons/hands_wear.png'
import lowerBodyIcon from '../../assets/category-icons/lower_body.png'
import feetIcon from '../../assets/category-icons/feet.png'
import accessoriesIcon from '../../assets/category-icons/earring.png'
import skinIcon from '../../assets/category-icons/skin.png'

export const SHOP_PAGE_SIZE = 48

/** A category pill: `all`, or a key of the web Shop's SUBCAT_MAP. */
export type WearableCategory = 'all' | 'Head' | 'Upper Body' | 'Handwear' | 'Lower Body' | 'Feet' | 'Accessories' | 'Skins'

export const WEARABLE_CATEGORIES: readonly { key: WearableCategory; label: string; icon: string }[] = [
  { key: 'all', label: 'All', icon: allIcon },
  { key: 'Head', label: 'Head', icon: hairIcon },
  { key: 'Upper Body', label: 'Upper body', icon: upperBodyIcon },
  { key: 'Handwear', label: 'Handwear', icon: handsWearIcon },
  { key: 'Lower Body', label: 'Lower body', icon: lowerBodyIcon },
  { key: 'Feet', label: 'Feet', icon: feetIcon },
  { key: 'Accessories', label: 'Accessories', icon: accessoriesIcon },
  { key: 'Skins', label: 'Skins', icon: skinIcon }
]

export type WearableSort = Extract<ShopSort, 'newest' | 'cheapest' | 'most_expensive' | 'name'>

export const WEARABLE_SORTS: readonly { id: WearableSort; label: string }[] = [
  { id: 'newest', label: 'Newest' },
  { id: 'cheapest', label: 'Cheapest' },
  { id: 'most_expensive', label: 'Most expensive' },
  { id: 'name', label: 'Name (A-Z)' }
]

export interface WearableFilters {
  category: WearableCategory
  sort: WearableSort
  /** Trimmed and lowercased, as the web Shop sends it. */
  search: string
}

/** A search keeps the server's relevance order unless another sort was picked. */
export function serverSort(filters: WearableFilters): ShopSort {
  return filters.search !== '' && filters.sort === 'newest' ? 'relevance' : filters.sort
}

/** The next page's offset: the rows loaded so far, while the last page was full and short of the total. */
export function nextWearablesPage(last: ItemsPage<unknown>, pages: readonly ItemsPage<unknown>[]): number | undefined {
  const loaded = pages.reduce((n, page) => n + page.items.length, 0)
  return last.items.length === SHOP_PAGE_SIZE && loaded < last.total ? loaded : undefined
}

export const shopQueries = {
  /** `creatorSales` is the rollout flag the fetcher's row mapping reads (discounted prices), so it is part of the key. */
  wearables: (filters: WearableFilters, { creatorSales }: { creatorSales: boolean }) =>
    infiniteQueryOptions({
      queryKey: ['shop', 'wearables', filters, { creatorSales }] as const,
      queryFn: ({ pageParam, signal }) =>
        fetchShopItems(
          {
            category: 'wearable',
            wearableCategories: filters.category === 'all' ? undefined : SUBCAT_MAP[filters.category],
            search: filters.search || undefined,
            sortBy: serverSort(filters),
            onSale: true,
            first: SHOP_PAGE_SIZE,
            skip: pageParam
          },
          { signal }
        ),
      initialPageParam: 0,
      getNextPageParam: nextWearablesPage,
      staleTime: 30_000
    })
}

/** The wearables on sale for one filter set; a new filter set keeps showing the last one's rows until it answers. */
export function useShopWearables(filters: WearableFilters, { creatorSales, enabled }: { creatorSales: boolean; enabled: boolean }) {
  return useInfiniteQuery({ ...shopQueries.wearables(filters, { creatorSales }), enabled, placeholderData: keepPreviousData })
}
