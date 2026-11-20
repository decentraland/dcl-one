// The overview's rail reads, apart from the page so a client navigation's loader can send them without loading it.

import { fetchShopItems, fetchTrendingItems, type Opts } from "@shop/discovery/api";
import { shopFlags } from "../flags";
import { remoteFlagsPrimed } from "../lib/remoteFlags";
import { prefetchShopQuery } from "../query";
import { shopReadsAfter } from "../reads";

export const readListings = (opts: Opts) => fetchShopItems({ first: 12, sortBy: "newest", listingType: "primary" }, opts);
export const readTrending = (listingType: "primary" | undefined, opts: Opts) => fetchTrendingItems({ first: 12, listingType }, opts);
export const readDeals = (opts: Opts) => fetchShopItems({ first: 12, discounted: true, sortBy: "discount", listingType: "primary" }, opts);

/** The rails' query keys, the same for the page, this prefetch and the server render that primes them. */
export const RAIL_KEYS = {
  listings: ["overview-listings"],
  trending: (listingType: "primary" | undefined) => ["overview-trending", listingType ?? "all"],
  deals: ["overview-deals"],
} as const;

/**
 * Sends the rails' reads before the page mounts, under the keys it reads: a client navigation's loader calls it with
 * its flags read, so the rails travel with the route's own requests. Trending as a visitor without the Marketplace
 * sees it (the server render primes the same). Best Deals only once the flags say creator sales are on: deals read
 * while the switch is off are mapped without their sale prices, and would sit fresh at full price after it turns on.
 */
export function prefetchOverviewRails(flags: Promise<unknown>): void {
  const reads = shopReadsAfter(flags);
  prefetchShopQuery(RAIL_KEYS.listings, (signal) => readListings(reads(signal)));
  prefetchShopQuery(RAIL_KEYS.trending("primary"), (signal) => readTrending("primary", reads(signal)));
  const deals = () => {
    if (shopFlags().creatorSales) prefetchShopQuery(RAIL_KEYS.deals, (signal) => readDeals(reads(signal)));
  };
  if (remoteFlagsPrimed()) deals();
  else void flags.then(deals, () => {});
}
