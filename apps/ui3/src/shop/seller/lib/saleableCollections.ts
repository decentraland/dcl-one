// from: shop@94584a5c:app/src/lib/saleableCollections.ts
import type { PublishableItem } from './builder'
import type { CollectionSaleState } from './collections'

/**
 * One of the collection's creations, as the review step needs it.
 *
 * Three states, not two. A discount re-prices the Shop's own credit listings, so an item still quoted in
 * MANA is left out — but it is NOT unlisted, and telling a creator it is "not for sale" when they can see
 * it on sale a click away is worse than saying nothing. It has its own state so the review can name the
 * one thing that would bring it in: updating its price.
 */
export type SaleItem = {
  key: string
  name: string
  thumbnail: string
  /** Its Shop price in credits. Null unless `state` is 'discounted'. */
  priceCredits: number | null
  /** 'discounted' takes the sale, 'classic' is listed but quoted in MANA, 'unlisted' is not for sale. */
  state: 'discounted' | 'classic' | 'unlisted'
  remainingSupply: number
}

/** A collection the creator can put on sale: one of theirs with at least one item listed in the Shop. */
export type SaleableCollection = {
  contractAddress: string
  name: string
  listedCount: number
  /**
   * The highest listed price, for the example line. The highest rather than the cheapest because a 1-credit
   * item rounds any discount away and the example would read "1 credit sells for 1 credit". Null when unknown.
   */
  examplePriceCredits: number | null
  /** Every creation in it, listed or not — the review step has to account for both. */
  items: SaleItem[]
}

/**
 * The creator's collections as a discount sees them.
 *
 * Every item goes in, listed or not: the sale only re-prices the listed ones, and the review step has to be
 * able to say which of the rest it will leave alone. Sold-out items are the exception — there is no copy
 * left for a discount to re-price, so they belong in neither group.
 *
 * Offered wherever something is LISTED, in either currency. A collection sold entirely in MANA cannot take
 * a discount yet, but it is still offered one: the modal answers why and where to fix it, where an absent
 * button explains nothing. A collection with nothing listed at all stays out — there the answer is to list
 * something, which this flow is not about.
 */
export function toSaleableCollections(
  items: PublishableItem[],
  saleState: Record<string, CollectionSaleState> | undefined
): SaleableCollection[] {
  const byAddress = new Map<string, SaleableCollection>()

  for (const item of items) {
    if (item.remainingSupply <= 0) continue
    const sale = saleState?.[`${item.contractAddress}-${item.blockchainItemId}`]
    // Three states, because "on sale" is not one thing here: a discount re-prices the Shop's own credit
    // listings, so an item still quoted in MANA cannot take one — but it IS listed, and the review has to
    // say that rather than call it not for sale.
    const state = !sale?.isOnSale ? 'unlisted' : sale.manaWei ? 'classic' : 'discounted'
    const listedInCredits = state === 'discounted'
    const key = item.contractAddress.toLowerCase()
    const entry = byAddress.get(key) ?? {
      contractAddress: key,
      name: item.collectionName,
      listedCount: 0,
      examplePriceCredits: null,
      items: []
    }
    entry.items.push({
      key: `${key}-${item.blockchainItemId}`,
      name: item.name,
      thumbnail: item.thumbnail,
      priceCredits: listedInCredits && sale ? sale.priceCredits : null,
      state,
      remainingSupply: item.remainingSupply
    })
    if (listedInCredits && sale) {
      entry.listedCount += 1
      entry.examplePriceCredits = Math.max(entry.examplePriceCredits ?? 0, sale.priceCredits)
    }
    byAddress.set(key, entry)
  }

  return [...byAddress.values()].filter(c => c.listedCount > 0 || c.items.some(i => i.state === 'classic'))
}
