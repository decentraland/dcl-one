// from: shop@94584a5c:app/src/lib/chip-links.ts
// Browse links for the detail page's attribute chips: only attributes the grid can filter by get one,
// spelled the way the grid writes its own URL. Gender has no filter, so it has no link.

import type { CatalogItem } from "./catalogItem";
import { SUBCAT_MAP } from "./categories";
import { SHOP_BASE } from "../../routes";

const SLOT_TO_SUBCATEGORY = new Map<string, string>(
  Object.entries(SUBCAT_MAP).flatMap(([key, slots]) => (slots.length === 1 ? [[slots[0]!, key] as [string, string]] : [])),
);

const BROWSE = `${SHOP_BASE}/items`;

export function rarityHref(rarity: string | undefined | null): string | null {
  if (!rarity) return null;
  return `${BROWSE}?rarities=${encodeURIComponent(rarity.toLowerCase())}`;
}

export function categoryHref(item: Pick<CatalogItem, "category" | "wearableCategory">): string | null {
  const slot = item.wearableCategory;
  if (slot) {
    const subCategory = SLOT_TO_SUBCATEGORY.get(slot);
    return subCategory
      ? `${BROWSE}?category=${item.category === "emote" ? "emote" : "wearable"}&subCategory=${encodeURIComponent(subCategory)}`
      : `${BROWSE}?category=${item.category === "emote" ? "emote" : "wearable"}`;
  }
  if (item.category === "emote") return `${BROWSE}?category=emote`;
  if (item.category === "wearable") return `${BROWSE}?category=wearable`;
  return null;
}

export function smartHref(): string {
  return `${BROWSE}?smart=true`;
}
