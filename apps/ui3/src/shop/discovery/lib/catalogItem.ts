// from: shop@94584a5c:app/src/lib/catalogItem.ts, app/src/lib/api.ts (the CatalogItem type, toGender)

import type { ShopCoupon } from "@catalyst-api/market/ShopCoupon";

export type Gender = "male" | "female" | "unisex" | null;
export type ListingAcquisition = "trade" | "store";
export type ListingSource = "native" | "legacy";
export type ListingType = "primary" | "secondary";

export type CatalogItem = {
  id: string;
  name: string;
  creator: string;
  contractAddress: string;
  itemId: string | null;
  category: string;
  wearableCategory?: string;
  rarity: string;
  network: string;
  chainId: number;
  thumbnail: string;
  urn?: string;
  priceCredits: number;
  manaWei?: string | null;
  gender: Gender;
  isSmart: boolean;
  emoteLoop?: boolean | null;
  emoteHasSound?: boolean;
  emoteHasProps?: boolean;
  tradeId?: string;
  tokenId?: string;
  acquisition?: ListingAcquisition;
  source?: ListingSource;
  listingType?: ListingType;
  issuedId?: string;
  seller?: string;
  available?: number;
  hasPrimaryListing?: boolean;
  listingCount?: number;
  compareAtCredits?: number;
  saleEndsAt?: number;
  coupon?: ShopCoupon;
  saleUnitsLeft?: number;
  onSale?: boolean;
};

export type RawCollectionItem = {
  id: string;
  name: string;
  creator?: string;
  contractAddress: string;
  itemId?: string | null;
  category: string;
  rarity?: string;
  network: string;
  chainId: number;
  thumbnail?: string;
  urn?: string;
  priceCredits?: number;
  price?: string | null;
  tradeId?: string | null;
  available?: string | number | null;
  isOnSale?: boolean;
  data?: {
    wearable?: { category?: string; bodyShapes?: string[]; isSmart?: boolean; description?: string };
    emote?: { category?: string; loop?: boolean; hasSound?: boolean; hasGeometry?: boolean; description?: string };
  };
};

export function toGender(bodyShapes?: string[]): Gender {
  if (!bodyShapes || bodyShapes.length === 0) return null;
  const male = bodyShapes.some((b) => b.includes("Male"));
  const female = bodyShapes.some((b) => b.includes("Female"));
  if (male && female) return "unisex";
  if (male) return "male";
  if (female) return "female";
  return null;
}

function toAvailable(value: string | number | null | undefined): number | undefined {
  if (value === null || value === undefined || value === "") return undefined;
  const n = Number(value);
  return Number.isInteger(n) && n >= 0 ? n : undefined;
}

export function toCatalogItem(r: RawCollectionItem): CatalogItem {
  const mana = r.tradeId == null && r.isOnSale && r.price && r.price !== "0" ? r.price : null;
  return {
    id: r.id,
    name: r.name,
    creator: r.creator ?? "",
    contractAddress: r.contractAddress,
    itemId: r.itemId ?? null,
    urn: r.urn,
    category: r.category,
    wearableCategory: r.data?.wearable?.category ?? r.data?.emote?.category,
    emoteLoop: r.data?.emote?.loop,
    emoteHasSound: r.data?.emote?.hasSound,
    emoteHasProps: r.data?.emote?.hasGeometry,
    rarity: r.rarity ?? "common",
    isSmart: !!r.data?.wearable?.isSmart,
    network: r.network,
    chainId: r.chainId,
    thumbnail: r.thumbnail ?? "",
    priceCredits: r.priceCredits ?? 0,
    ...(r.tradeId ? { tradeId: r.tradeId } : {}),
    ...(mana ? { manaWei: mana } : {}),
    available: toAvailable(r.available),
    onSale: r.isOnSale ?? undefined,
    gender: toGender(r.data?.wearable?.bodyShapes),
  };
}
