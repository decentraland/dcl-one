// Which of a collection's listings a new discount re-prices. A coupon is redeemed by the marketplace a listing was
// signed for, and each marketplace version trusts its own coupon manager, so only orders on the marketplace whose
// manager signs new discounts take one. A collection-store price is bought through CollectionStore, which takes no
// coupon at all.
import {
  DISCOUNT_TYPE_RATE,
  getCouponContracts,
  getCouponManagerForMarketplace,
  liveDiscountStatus,
  type CollectionDiscount
} from '~/lib/coupons'
import { type ItemListing } from '~/lib/listings'

export type DiscountReachOptions = {
  chainId: number
  /** Whether the Shop discounts MANA listings for buyers (`shop-legacy-coupons`); off, only credits listings do. */
  manaListings: boolean
}

/** Whether a new discount re-prices this listing for Shop buyers. */
export function takesDiscount(listing: ItemListing, { chainId, manaListings }: DiscountReachOptions): boolean {
  if (!listing.tradeId || !listing.tradeContract) return false
  if (listing.currency === 'mana' && (!manaListings || listing.manaWei === 0n)) return false
  const manager = getCouponManagerForMarketplace(listing.tradeContract, chainId)
  const signing = getCouponContracts(chainId)?.couponManager.address.toLowerCase()
  return manager !== null && manager === signing
}

const PPM = 1_000_000

/** What a credits price rings up at with a discount of `ppm`, rounded up to a whole credit the way checkout does. */
export function discountedCreditsPpm(credits: number, ppm: number): number {
  return Math.max(1, Math.ceil((credits * (PPM - ppm)) / PPM))
}

/** What a MANA price rings up at with a discount of `ppm`: the contract's `price - floor(price * ppm / 1e6)`. */
export function discountedManaWeiPpm(manaWei: bigint, ppm: number): bigint {
  return manaWei - (manaWei * BigInt(ppm)) / BigInt(PPM)
}

/** {@link discountedCreditsPpm} for a whole percent. */
export function discountedCredits(credits: number, pct: number): number {
  return discountedCreditsPpm(credits, pct * 10_000)
}

/** {@link discountedManaWeiPpm} for a whole percent. */
export function discountedManaWei(manaWei: bigint, pct: number): bigint {
  return discountedManaWeiPpm(manaWei, pct * 10_000)
}

/**
 * The deepest running discount a buyer gets on this listing, in parts per million, or undefined. Unlike
 * {@link takesDiscount}, which asks about a discount yet to be signed, each running discount is matched against
 * the manager it was signed with: a listing only redeems coupons of the manager its marketplace trusts.
 */
export function runningDiscountPpm(
  listing: ItemListing,
  discounts: CollectionDiscount[],
  { chainId, manaListings }: DiscountReachOptions,
  now = Date.now()
): number | undefined {
  if (!listing.tradeId || !listing.tradeContract) return undefined
  if (listing.currency === 'mana' && (!manaListings || listing.manaWei === 0n)) return undefined
  const manager = getCouponManagerForMarketplace(listing.tradeContract, chainId)
  if (!manager) return undefined
  const reaching = discounts.filter(
    discount =>
      discount.discountType === DISCOUNT_TYPE_RATE &&
      discount.couponManager.toLowerCase() === manager &&
      liveDiscountStatus(discount, now) === 'active'
  )
  return reaching.length > 0 ? Math.max(...reaching.map(discount => discount.discount)) : undefined
}
