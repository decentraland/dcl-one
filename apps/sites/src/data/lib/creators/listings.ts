// Primary (mint) listings of a published collection, from marketplace-server. `/v1/items` says which
// items are on sale and their wei price (on-chain store price, or the trade amount); the shop catalog
// says which of those are credits listings, whose trade amount is USD wei rather than MANA wei.
import { maxUint256 } from 'viem'
import { config } from '~/config'

/**
 * `tradeId` identifies the off-chain order behind the listing and `tradeContract` the marketplace it was signed for
 * (lower-cased); a legacy CollectionStore price has neither.
 */
export type ItemListing = { itemId: string; tradeId?: string; tradeContract?: string } & (
  { currency: 'credits'; credits: number } | { currency: 'mana'; manaWei: bigint }
)

type CatalogRow = { itemId: string | null; priceCredits: number; source: 'native' | 'legacy' }
type ItemRow = {
  itemId: string
  price: string
  isOnSale: boolean
  tradeId?: string | null
  tradeContractAddress?: string | null
}

// A collection has at most a few dozen items; the catalog caps `first` at 1000.
const FIRST = '1000'
// The legacy builder's "no price set" marker on a store-minted item.
const NO_PRICE = maxUint256.toString()
const WEI = /^\d+$/

async function getJson<T>(path: string, query: Record<string, string>): Promise<T> {
  const response = await fetch(`${config.get('MARKETPLACE_SERVER_URL')}${path}?${new URLSearchParams(query)}`)
  if (!response.ok) throw new Error(`marketplace-server request failed: ${path} (${response.status})`)
  return (await response.json()) as T
}

/** The id of the item's current primary order, or null when it has none (or only a legacy store price). */
export async function fetchItemTradeId(contractAddress: string, itemId: string): Promise<string | null> {
  const catalog = await getJson<{ data?: { tradeId: string | null }[] }>('/v3/catalog/unified', {
    contractAddress,
    itemId,
    listingType: 'primary',
    first: '5'
  })
  return catalog.data?.find(row => row.tradeId)?.tradeId ?? null
}

/** The collection's primary listings keyed by on-chain item id. Items not on sale (or sold out) are absent. */
export async function fetchCollectionListings(contractAddress: string): Promise<Map<string, ItemListing>> {
  const [items, catalog] = await Promise.all([
    getJson<{ data?: ItemRow[] }>('/v1/items', { contractAddress, first: FIRST }),
    getJson<{ data?: CatalogRow[] }>('/v3/catalog/unified', { contractAddress, listingType: 'primary', first: FIRST })
  ])
  const credits = new Map<string, number>()
  for (const row of catalog.data ?? []) {
    if (row.source === 'native' && row.itemId !== null) credits.set(row.itemId, row.priceCredits)
  }
  const listings = new Map<string, ItemListing>()
  for (const { itemId, price, isOnSale, tradeId, tradeContractAddress } of items.data ?? []) {
    if (!isOnSale || price === NO_PRICE || !WEI.test(price)) continue
    const inCredits = credits.get(itemId)
    const order = tradeId
      ? { tradeId, ...(tradeContractAddress ? { tradeContract: tradeContractAddress.toLowerCase() } : {}) }
      : {}
    listings.set(
      itemId,
      inCredits === undefined
        ? { itemId, ...order, currency: 'mana', manaWei: BigInt(price) }
        : { itemId, ...order, currency: 'credits', credits: inCredits }
    )
  }
  return listings
}
