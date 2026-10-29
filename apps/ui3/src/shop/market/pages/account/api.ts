// from: marketplace@3bf3662f:webapp/src/modules/vendor/decentraland/nft/api.ts (the /v1/nfts browse query an
//       account's Collectibles view sends), webapp/src/modules/routing/types.ts (the Section values)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import type { NftResult } from "@catalyst-api/market/NftResult";
import { shopClient, type ShopClient } from "../../../api/shop";
import type { Gender } from "../../../discovery/lib/catalogItem";
import { toMyAsset, type MyAsset } from "../../lib/api";

export type AccountNft = MyAsset & {
  wearableCategory?: string;
  gender: Gender;
  isSmart: boolean;
  parcel?: { x: string; y: string };
  estateSize?: number;
};

export type AssetType = "nft" | "item";

export type Section =
  | "all"
  | "wearables"
  | "wearables_head"
  | "wearables_upper_body"
  | "wearables_hands"
  | "wearables_lower_body"
  | "wearables_feet"
  | "wearables_accessories"
  | "wearables_skin"
  | "emotes"
  | "ens"
  | "land"
  | "on_rent";

export const WEARABLE_SECTIONS: { key: Section; label: string; icon: string; categories: string[] }[] = [
  { key: "wearables_head", label: "Head", icon: "cat-head", categories: ["eyebrows", "eyes", "facial_hair", "hair", "mouth"] },
  { key: "wearables_upper_body", label: "Upper Body", icon: "cat-upper", categories: ["upper_body"] },
  { key: "wearables_hands", label: "Handwear", icon: "cat-handwear", categories: ["hands_wear"] },
  { key: "wearables_lower_body", label: "Lower Body", icon: "cat-lower", categories: ["lower_body"] },
  { key: "wearables_feet", label: "Feet", icon: "cat-feet", categories: ["feet"] },
  { key: "wearables_accessories", label: "Accessories", icon: "cat-accessories", categories: ["earring", "eyewear", "hat", "helmet", "mask", "tiara", "top_head"] },
  { key: "wearables_skin", label: "Skins", icon: "cat-skins", categories: ["skin"] },
];

const SECTIONS = new Set<string>(["all", "wearables", "emotes", "ens", "land", "on_rent", ...WEARABLE_SECTIONS.map((s) => s.key)]);

export function parseSection(raw: string | null, assetType: AssetType): Section {
  const s = raw === "names" ? "ens" : raw === "parcels" || raw === "estates" ? "land" : raw;
  if (s && SECTIONS.has(s)) {
    if (assetType === "item" && (s === "all" || s === "ens" || s === "land" || s === "on_rent")) return "wearables";
    return s as Section;
  }
  return assetType === "item" ? "wearables" : "all";
}

export const isWearableSection = (s: Section) => s === "wearables" || s.startsWith("wearables_");

export function wearableCategoriesOf(s: Section): string[] | undefined {
  return WEARABLE_SECTIONS.find((w) => w.key === s)?.categories;
}

type SortOption = { value: string; label: string };

export function sortOptions(assetType: AssetType, onlyOnSale: boolean): SortOption[] {
  const base = [
    { value: "newest", label: "Newest" },
    { value: "name", label: "Name" },
  ];
  if (assetType === "item" || onlyOnSale) {
    return [{ value: "recently_listed", label: "Recently listed" }, { value: "recently_sold", label: "Recently sold" }, { value: "cheapest", label: "Cheapest" }, ...base];
  }
  return base;
}

function genderOf(bodyShapes: string[] | undefined): Gender {
  if (!bodyShapes?.length) return null;
  const male = bodyShapes.some((b) => /BaseMale$/i.test(b));
  const female = bodyShapes.some((b) => /BaseFemale$/i.test(b));
  return male && female ? "unisex" : male ? "male" : female ? "female" : null;
}

function toAccountNft(r: NftResult): AccountNft {
  const data = r.nft.data;
  const base = toMyAsset(r);
  if ("wearable" in data) return { ...base, wearableCategory: data.wearable.category, gender: genderOf(data.wearable.bodyShapes), isSmart: data.wearable.isSmart };
  if ("emote" in data) return { ...base, wearableCategory: data.emote.category, gender: genderOf(data.emote.bodyShapes), isSmart: false };
  if ("parcel" in data) return { ...base, gender: null, isSmart: false, parcel: { x: data.parcel.x, y: data.parcel.y } };
  if ("estate" in data) return { ...base, gender: null, isSmart: false, estateSize: data.estate.size };
  return { ...base, gender: null, isSmart: false };
}

type AccountNftsQuery = {
  section: Section;
  first: number;
  skip: number;
  search?: string;
  sortBy: string;
  onlyOnSale: boolean;
  rarities?: string[];
};

export async function fetchAccountNfts(address: string, q: AccountNftsQuery, c: ShopClient = shopClient()): Promise<{ assets: AccountNft[]; total: number }> {
  const addr = address.toLowerCase();
  const query: Record<string, string | number | string[] | undefined> = {
    first: q.first,
    skip: q.skip,
    sortBy: q.sortBy,
    orderDirection: "desc",
    search: q.search || undefined,
    isOnSale: q.onlyOnSale ? "true" : undefined,
    itemRarity: q.rarities?.length ? q.rarities : undefined,
  };
  if (q.section === "on_rent") {
    Object.assign(query, { tenant: addr, isOnRent: "true", isLand: "true", rentalStatus: "executed", isOnSale: undefined });
  } else {
    query.owner = addr;
    if (q.section === "land") query.isLand = "true";
    else if (q.section === "ens") query.category = "ens";
    else if (q.section === "emotes") query.category = "emote";
    else if (isWearableSection(q.section)) {
      query.category = "wearable";
      query.wearableCategory = wearableCategoriesOf(q.section);
    }
  }
  const res = await c.market.nfts(query);
  return { assets: res.data.map(toAccountNft), total: res.total };
}
