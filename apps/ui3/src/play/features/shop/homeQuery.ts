// The Shop's All home: the published outfits, what is trending and what is new, read through the web
// Shop's client and fetchers (shop/api, shop/discovery/api) behind TanStack key and options factories.

import { queryOptions, useQuery } from '@tanstack/react-query'
import { isShopApiError } from '../../../shop/api/client'
import { fetchCatalogByIds, fetchShopItems, fetchTrendingItems } from '../../../shop/discovery/api'
import { listingTypeFor } from '../../../shop/market/access-rule'
import { shopClient } from '../../../shop/api/host'

export const HOME_ROW_SIZE = 12

export interface OutfitSummary {
  id: string
  name: string
  /** The outfit's picture, or null without one. */
  image: string | null
  items: { contractAddress: string; itemId: string }[]
  gradientFrom: string
  gradientTo: string
}

const HEX = /^#[0-9a-f]{6}$/i

/** The published outfits, as the web Shop's Overview reads them; none when the node serves none. */
export async function fetchOutfits(signal?: AbortSignal): Promise<OutfitSummary[]> {
  let body: unknown
  try {
    body = await shopClient().outfits.list(undefined, { signal })
  } catch (e) {
    if (isShopApiError(e) && (e.status === 404 || e.code === 'noLegacyAlias')) return []
    throw e
  }
  const list = body != null && typeof body === 'object' && 'outfits' in body ? (body as { outfits: unknown }).outfits : body
  if (!Array.isArray(list)) return []
  const out: OutfitSummary[] = []
  for (const raw of list as Partial<{ id: string; name: string; thumbnailHash: string; items: unknown[]; gradientFrom: string; gradientTo: string; published: boolean }>[]) {
    if (raw == null || typeof raw.id !== 'string' || typeof raw.name !== 'string' || !Array.isArray(raw.items) || raw.published === false) continue
    const items = (raw.items as Partial<{ contractAddress: string; itemId: string }>[]).filter(
      (i): i is { contractAddress: string; itemId: string } => i != null && typeof i.contractAddress === 'string' && typeof i.itemId === 'string'
    )
    out.push({
      id: raw.id,
      name: raw.name,
      image: raw.thumbnailHash ? shopClient().outfits.thumbnailUrl(raw.thumbnailHash) : null,
      items,
      gradientFrom: HEX.test(raw.gradientFrom ?? '') ? (raw.gradientFrom as string) : '',
      gradientTo: HEX.test(raw.gradientTo ?? '') ? (raw.gradientTo as string) : ''
    })
  }
  return out
}

export const homeQueries = {
  outfits: () => queryOptions({ queryKey: ['shop', 'home', 'outfits'] as const, queryFn: ({ signal }) => fetchOutfits(signal), staleTime: 60_000 }),
  /** An outfit's items, to dress the avatar in it. */
  outfitItems: (outfit: OutfitSummary) =>
    queryOptions({
      // The items are inputs too: an outfit republished under the same id lists others.
      queryKey: ['shop', 'home', 'outfit-items', outfit.id, outfit.items.map((i) => `${i.contractAddress.toLowerCase()}-${i.itemId}`)] as const,
      queryFn: ({ signal }) => fetchCatalogByIds(outfit.items.map((i) => `${i.contractAddress.toLowerCase()}-${i.itemId}`), { client: shopClient(), signal }),
      staleTime: 60_000
    }),
  /**
   * `creatorSales` is the rollout the rows' mapping reads (discounted prices), so it is part of the key. Without
   * `marketplace` (its rollout, for a player who is not a guest) the rows are creators' primary sales only: a resale is
   * the Marketplace's.
   */
  trending: ({ creatorSales, marketplace }: Rollouts) =>
    queryOptions({
      queryKey: ['shop', 'home', 'trending', { creatorSales, marketplace }] as const,
      queryFn: ({ signal }) => fetchTrendingItems({ first: HOME_ROW_SIZE, listingType: listingTypeFor(marketplace) }, { client: shopClient(), signal }),
      staleTime: 60_000
    }),
  newest: ({ creatorSales, marketplace }: Rollouts) =>
    queryOptions({
      queryKey: ['shop', 'home', 'newest', { creatorSales, marketplace }] as const,
      queryFn: async ({ signal }) => (await fetchShopItems({ sortBy: 'newest', onSale: true, first: HOME_ROW_SIZE, listingType: listingTypeFor(marketplace) }, { client: shopClient(), signal })).items,
      staleTime: 60_000
    })
}

export type Rollouts = { creatorSales: boolean; marketplace: boolean }

export const useShopOutfits = (enabled: boolean) => useQuery({ ...homeQueries.outfits(), enabled })
export const useTrending = (rollouts: Rollouts, enabled: boolean) => useQuery({ ...homeQueries.trending(rollouts), enabled })
export const useNewest = (rollouts: Rollouts, enabled: boolean) => useQuery({ ...homeQueries.newest(rollouts), enabled })
