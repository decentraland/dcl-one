// from: shop@94584a5c:app/src/lib/suggestionSeeds.ts
// What this browser can tell the recommender that the server cannot look up: the cart first (the
// strongest intent), then recently viewed. Favourites are not seeds: the server reads them itself for
// a signed request.

import type { CatalogItem } from "./catalogItem";

export const MAX_SEEDS = 20;

export function seedIdOf(item: Pick<CatalogItem, "contractAddress" | "itemId">): string | null {
  if (!item.contractAddress || !item.itemId) return null;
  return `${item.contractAddress.toLowerCase()}-${item.itemId}`;
}

export function buildSuggestionSeeds(sources: {
  cart?: Array<Pick<CatalogItem, "contractAddress" | "itemId">>;
  recentlyViewed?: Array<Pick<CatalogItem, "contractAddress" | "itemId">>;
}): string[] {
  const seeds: string[] = [];
  const seen = new Set<string>();
  for (const group of [sources.cart, sources.recentlyViewed]) {
    for (const item of group ?? []) {
      const id = seedIdOf(item);
      if (!id || seen.has(id)) continue;
      seen.add(id);
      seeds.push(id);
      if (seeds.length >= MAX_SEEDS) return seeds;
    }
  }
  return seeds;
}

/** An order-independent key for the seed set. */
export function seedsKey(seeds: string[]): string {
  return [...seeds].sort().join(",");
}
