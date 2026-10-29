// from: shop@94584a5c:app/src/lib/recently-viewed.ts
// The browser's recently viewed items (localStorage, no backend): most recent first, deduped by id,
// capped. Stored for display and as suggestion seeds; never read as a price.

import { useEffect, useState } from "react";

import type { CatalogItem } from "./catalogItem";

const KEY = "shop:recently-viewed";
const MAX = 12;

export function getRecentlyViewed(): CatalogItem[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const items = JSON.parse(raw) as CatalogItem[];
    return Array.isArray(items) ? items.filter((i) => i && typeof i.id === "string").slice(0, MAX) : [];
  } catch {
    return [];
  }
}

// Empty through server render and hydration (the server has no localStorage), then this browser's list.
export function useRecentlyViewed(): CatalogItem[] {
  const [items, setItems] = useState<CatalogItem[]>([]);
  useEffect(() => setItems(getRecentlyViewed()), []);
  return items;
}

export function recordViewed(item: CatalogItem): void {
  if (!item?.id) return;
  try {
    const next = [item, ...getRecentlyViewed().filter((i) => i.id !== item.id)].slice(0, MAX);
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    // Recently viewed is a convenience.
  }
}
