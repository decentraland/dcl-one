// A collection as the discount modal reads it, ported from the Shop's lib/saleableCollections: every item it has,
// split into what a new discount re-prices, what stays at a price it can't change, and what isn't for sale.
import { getContentsStorageUrl } from '~/lib/builder'
import {
  discountedCredits,
  discountedManaWei,
  takesDiscount,
  type DiscountReachOptions
} from '~/lib/collectionDiscount'
import { type Collection } from '~/lib/collections'
import { getItemSales, type Item } from '~/lib/items'
import { creditsForUsdWei } from '~/lib/publishFee'
import { type ItemListing } from '~/lib/listings'

export type SaleItem = {
  key: string
  name: string
  thumbnail: string
  /** Its Shop price in credits, for a discounted item; null when unknown (a MANA price with no rate yet) or not discounted. */
  priceCredits: number | null
  /** Its MANA price, for a discounted MANA listing. */
  manaWei?: bigint
  /** 'discounted' takes the discount, 'classic' is listed at a price it can't change, 'unlisted' is not for sale. */
  state: 'discounted' | 'classic' | 'unlisted'
  /** A MANA listing the discount reaches. */
  mana?: boolean
  remainingSupply: number
}

export type SaleableCollection = {
  contractAddress: string
  collectionId: string
  itemCount: number
  name: string
  /** Listings a new discount re-prices. */
  listedCount: number
  /** USD wei per MANA its MANA prices were quoted in credits at, so a discounted one is quoted the same way. */
  manaRate?: bigint
  items: SaleItem[]
}

/** Credits for a MANA amount at `rate` USD wei per MANA, rounded up to a whole credit (1 credit = $0.10) like the Shop. */
export function manaToCredits(manaWei: bigint, rate: bigint): number {
  return Math.max(1, creditsForUsdWei((manaWei * rate) / 10n ** 18n))
}

export type SaleableOptions = DiscountReachOptions & {
  /** USD wei per MANA; without it a discounted MANA listing has no credits price to preview. */
  manaRate?: bigint
}

/** The collection's items, sold-out ones aside (nothing left for a discount to re-price), as the modal reads them. */
export function toSaleableCollection(
  collection: Collection,
  items: Item[],
  listings: Map<string, ItemListing>,
  options: SaleableOptions
): SaleableCollection {
  const saleItems: SaleItem[] = []
  for (const item of items) {
    const sales = getItemSales(item)
    const remainingSupply = sales ? sales.maxSupply - sales.minted : 0
    if (remainingSupply <= 0) continue
    const listing = item.tokenId ? listings.get(item.tokenId) : undefined
    const thumbnailHash = item.contents[item.thumbnail]
    const base = {
      key: `${collection.contractAddress}-${item.tokenId ?? item.id}`,
      name: item.name,
      thumbnail: thumbnailHash ? getContentsStorageUrl(thumbnailHash) : '',
      remainingSupply
    }
    if (!listing) {
      saleItems.push({ ...base, priceCredits: null, state: 'unlisted' })
    } else if (!takesDiscount(listing, options)) {
      saleItems.push({ ...base, priceCredits: null, state: 'classic' })
    } else if (listing.currency === 'credits') {
      saleItems.push({ ...base, priceCredits: listing.credits, state: 'discounted' })
    } else {
      const priceCredits = options.manaRate ? manaToCredits(listing.manaWei, options.manaRate) : null
      saleItems.push({ ...base, priceCredits, manaWei: listing.manaWei, state: 'discounted', mana: true })
    }
  }
  return {
    contractAddress: collection.contractAddress ?? '',
    collectionId: collection.id,
    itemCount: collection.itemCount,
    name: collection.name,
    listedCount: saleItems.filter(item => item.state === 'discounted').length,
    manaRate: options.manaRate,
    items: saleItems
  }
}

/**
 * What a discounted item sells for in credits, the way a buyer will see it. A MANA listing is discounted in MANA
 * first, as the contract does, and that sale amount converted at the same rate: discounting its already rounded
 * credits price can quote a credit more than the coupon yields. Null when it has no credits price to quote.
 */
export function discountedPriceCredits(item: SaleItem, pct: number, manaRate: bigint | undefined): number | null {
  if (item.manaWei !== undefined && manaRate) return manaToCredits(discountedManaWei(item.manaWei, pct), manaRate)
  return item.priceCredits === null ? null : discountedCredits(item.priceCredits, pct)
}
