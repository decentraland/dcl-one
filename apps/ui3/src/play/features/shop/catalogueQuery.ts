// The in-world Shop's catalogue grid (wearables or emotes): the web Shop's fetchers
// (shop/discovery/api), category map and sorts behind one TanStack key and options factory, so every
// reader of one filter set and page shares one cache entry. As the web Shop's browse grid does
// (shop/discovery/lib/itemsGrid), what is on sale reads the listings feed and anything wider the
// catalogue; the card prices each row by the Shop's own rule (library/shop discovery/cardPrice).

import { queryOptions, useQuery } from '@tanstack/react-query'
import { fetchCatalogItems, fetchShopItems, type ShopSort } from '../../../shop/discovery/api'
import { SUBCAT_MAP } from '../../../shop/discovery/lib/categories'
// The design's ALL pill: the icon library's 3×3 `apps` grid (16px of 4px squares in the 26px slot).
import allIcon from '../../assets/shop/all.svg'
import hairIcon from '../../assets/category-icons/hair.png'
import upperBodyIcon from '../../assets/category-icons/upper_body.png'
import handsWearIcon from '../../assets/category-icons/hands_wear.png'
import lowerBodyIcon from '../../assets/category-icons/lower_body.png'
import feetIcon from '../../assets/category-icons/feet.png'
import accessoriesIcon from '../../assets/category-icons/earring.png'
import skinIcon from '../../assets/category-icons/skin.png'
import { listingTypeFor } from '@shop/market/access-rule'
import { shopReads } from '../../../shop/discovery/reads'

export const SHOP_PAGE_SIZE = 48

export type CatalogueKind = 'wearable' | 'emote'
export type CatalogueStatus = 'all' | 'on_sale' | 'not_for_sale'

/** A category pill: `all`, or a key of the web Shop's SUBCAT_MAP. */
export interface CategoryPill {
  key: string
  label: string
  icon?: string
}

export const WEARABLE_CATEGORIES: readonly CategoryPill[] = [
  { key: 'all', label: 'All', icon: allIcon },
  { key: 'Head', label: 'Head', icon: hairIcon },
  { key: 'Upper Body', label: 'Upper body', icon: upperBodyIcon },
  { key: 'Handwear', label: 'Handwear', icon: handsWearIcon },
  { key: 'Lower Body', label: 'Lower body', icon: lowerBodyIcon },
  { key: 'Feet', label: 'Feet', icon: feetIcon },
  { key: 'Accessories', label: 'Accessories', icon: accessoriesIcon },
  { key: 'Skins', label: 'Skins', icon: skinIcon }
]

export const EMOTE_CATEGORIES: readonly CategoryPill[] = [
  { key: 'all', label: 'All', icon: allIcon },
  ...['Dance', 'Stunt', 'Greetings', 'Fun', 'Poses', 'Reactions', 'Horror', 'Miscellaneous'].map((key) => ({ key, label: key }))
]

export const categoriesFor = (kind: CatalogueKind): readonly CategoryPill[] => (kind === 'emote' ? EMOTE_CATEGORIES : WEARABLE_CATEGORIES)

export type CatalogueSort = Extract<ShopSort, 'newest' | 'cheapest' | 'most_expensive' | 'name'>

export const CATALOGUE_SORTS: readonly { id: CatalogueSort; label: string }[] = [
  { id: 'newest', label: 'Newest' },
  { id: 'cheapest', label: 'Cheapest' },
  { id: 'most_expensive', label: 'Most expensive' },
  { id: 'name', label: 'Name (A-Z)' }
]

export interface CatalogueFilters {
  kind: CatalogueKind
  category: string
  sort: CatalogueSort
  /** Trimmed and lowercased, as the web Shop sends it. */
  search: string
  /** Sorted, so one set of rarities is one key. */
  rarities: string[]
  minCredits: number | null
  maxCredits: number | null
  /** The status picked in the filters; null until one is, which reads as the default for the search (`statusOf`). */
  status: CatalogueStatus | null
  /** Creator sales only: needs the creator-sales rollout. */
  deals: boolean
  smart: boolean
}

export const defaultFilters = (kind: CatalogueKind): CatalogueFilters => ({
  kind,
  category: 'all',
  sort: 'newest',
  search: '',
  rarities: [],
  minCredits: null,
  maxCredits: null,
  status: null,
  deals: false,
  smart: false
})

/** A search keeps the server's relevance order unless another sort was picked. */
export function serverSort(filters: Pick<CatalogueFilters, 'search' | 'sort'>): ShopSort {
  return filters.search !== '' && filters.sort === 'newest' ? 'relevance' : filters.sort
}

/** The web Shop's default status (its items grid's `defaultStatusFor`): what is on sale, or everything while searching. */
export const defaultStatus = (search: string): CatalogueStatus => (search !== '' ? 'all' : 'on_sale')

/** The status the catalogue reads: the one picked in the filters, else the default for the search. */
export const statusOf = (filters: Pick<CatalogueFilters, 'status' | 'search'>): CatalogueStatus => filters.status ?? defaultStatus(filters.search)

/** Deals are on-sale listings, so they read the listings feed whatever the status says. */
export const readsListings = (filters: CatalogueFilters, creatorSales: boolean): boolean => statusOf(filters) === 'on_sale' || (creatorSales && filters.deals)

export const pageCount = (total: number, size: number = SHOP_PAGE_SIZE): number => Math.max(1, Math.ceil(total / size))

export const shopQueries = {
  /**
   * `creatorSales` is the rollout the fetchers' row mapping reads (discounted prices), so it is part of the key, as is
   * `marketplace`: without it the listings are creators' primary sales only.
   */
  catalogue: (filters: CatalogueFilters, page: number, { creatorSales, marketplace = false }: { creatorSales: boolean; marketplace?: boolean }) =>
    queryOptions({
      // Keyed by the status it reads, so the default and the same status picked share their pages.
      queryKey: ['shop', 'catalogue', { ...filters, status: statusOf(filters) }, page, { creatorSales, marketplace }] as const,
      queryFn: ({ signal }) => {
        const categories = filters.category === 'all' ? undefined : SUBCAT_MAP[filters.category]
        const rarities = filters.rarities.length > 0 ? filters.rarities : undefined
        const search = filters.search || undefined
        const sortBy = serverSort(filters)
        const smart = filters.kind === 'wearable' && filters.smart ? true : undefined
        const paging = { first: SHOP_PAGE_SIZE, skip: page * SHOP_PAGE_SIZE }
        if (readsListings(filters, creatorSales))
          return fetchShopItems(
            {
              category: filters.kind,
              wearableCategories: categories,
              rarities,
              search,
              sortBy,
              isSmart: smart,
              onSale: true,
              listingType: listingTypeFor(marketplace),
              discounted: creatorSales && filters.deals ? true : undefined,
              minPriceCredits: filters.minCredits ?? undefined,
              maxPriceCredits: filters.maxCredits ?? undefined,
              ...paging
            },
            shopReads(signal)
          )
        // Nothing not for sale has a price, so the range applies to the other statuses only.
        const status = statusOf(filters)
        const priced = status !== 'not_for_sale'
        return fetchCatalogItems(
          {
            category: filters.kind,
            wearableCategories: categories,
            rarities,
            search,
            sortBy,
            isWearableSmart: smart,
            isOnSale: status === 'not_for_sale' ? false : undefined,
            minPriceCredits: priced ? (filters.minCredits ?? undefined) : undefined,
            maxPriceCredits: priced ? (filters.maxCredits ?? undefined) : undefined,
            ...paging
          },
          shopReads(signal)
        )
      },
      staleTime: 30_000
    })
}

/** One page of the catalogue for a filter set; a new page or filter set keeps the last rows until it answers. */
export function useShopCatalogue(filters: CatalogueFilters, page: number, { creatorSales, marketplace, enabled }: { creatorSales: boolean; marketplace: boolean; enabled: boolean }) {
  return useQuery({
    ...shopQueries.catalogue(filters, page, { creatorSales, marketplace }),
    enabled,
    // A new page or filter of the same kind shows the last rows, dimmed, until it answers; another kind
    // (wearables → emotes) has nothing in common with them, so it loads afresh.
    placeholderData: (previous, previousQuery) => (sameKind(previousQuery?.queryKey, filters.kind) ? previous : undefined)
  })
}

const sameKind = (key: readonly unknown[] | undefined, kind: CatalogueKind): boolean => (key?.[2] as CatalogueFilters | undefined)?.kind === kind
