// from: shop@94584a5c:app/src/lib/suggestions.ts
// The detail page's rail: the anchor's collection first (never truncated), padded to a target with the
// creator's other items and then similar ones. Identity is (contract, item), not the feed's `id`.

import type { CatalogItem } from "./catalogItem";

export const SUGGESTIONS_TARGET = 15;

export type SuggestionAnchor = { id?: string; contractAddress?: string; itemId?: string | null; tokenId?: string };

export type SuggestionTiers = { collection: CatalogItem[]; creator?: CatalogItem[]; related?: CatalogItem[] };

export type MergedSuggestions = { items: CatalogItem[]; isCollectionOnly: boolean };

function identity(item: CatalogItem): string {
  return `${item.contractAddress}-${item.itemId ?? item.tokenId ?? item.id}`.toLowerCase();
}

function sameContract(item: CatalogItem, anchor: SuggestionAnchor): boolean {
  if (!anchor.contractAddress) return false;
  return item.contractAddress?.toLowerCase() === anchor.contractAddress.toLowerCase();
}

function isAnchor(item: CatalogItem, anchor: SuggestionAnchor): boolean {
  if (anchor.id && item.id === anchor.id) return true;
  if (anchor.itemId && item.itemId === anchor.itemId && sameContract(item, anchor)) return true;
  if (anchor.tokenId && item.tokenId === anchor.tokenId && sameContract(item, anchor)) return true;
  return false;
}

export function mergeSuggestions(tiers: SuggestionTiers, anchor: SuggestionAnchor, target: number = SUGGESTIONS_TARGET): MergedSuggestions {
  const seen = new Set<string>();
  const items: CatalogItem[] = [];
  const take = (source: CatalogItem[], limit: number) => {
    for (const item of source) {
      if (items.length >= limit) return;
      if (isAnchor(item, anchor)) continue;
      const key = identity(item);
      if (seen.has(key)) continue;
      seen.add(key);
      items.push(item);
    }
  };
  take(tiers.collection, Infinity);
  const collectionCount = items.length;
  take(tiers.creator ?? [], target);
  take(tiers.related ?? [], target);
  return { items, isCollectionOnly: items.length > 0 && items.length === collectionCount };
}
