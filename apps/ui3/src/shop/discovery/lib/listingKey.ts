// from: shop@94584a5c:app/src/lib/listingKey.ts
// A stable, unique React key for a grid card: the listing source plus its trade or item identity, so a
// reordered page keeps each card on its own listing and a native and a legacy row never collide.

import type { CatalogItem } from "./catalogItem";

export function listingKey(item: CatalogItem): string {
  const source = item.source ?? "item";
  const identity = item.tradeId || `${item.contractAddress}-${item.tokenId ?? item.itemId ?? ""}`;
  return `${source}:${identity}`;
}
