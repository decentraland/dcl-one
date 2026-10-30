// from: shop@94584a5c:app/src/lib/api.ts (the catalogue feeds and their mappers), app/src/lib/collections.ts,
//       app/src/lib/rankings.ts, app/src/lib/search.ts;
//       marketplace@f3956826:webapp/src/modules/vendor/decentraland/{catalog,collection,item,nft,order,sale}/api.ts,
//       webapp/src/modules/{account,collection,contract,item,nft}/actions.ts (the FETCH_* reads)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import type { QueryParams } from "../../data/catalyst/client";
import type { ShopCoupon } from "@catalyst-api/market/ShopCoupon";
import type { SuggestionReason } from "@catalyst-api/market/SuggestionReason";
import { shopClient, type ShopClient } from "../api/shop";
import { shopFlags } from "./flags";
import { readFreshness, type IndexFreshness } from "./freshness";
import { toCatalogItem, type CatalogItem, type ListingAcquisition, type ListingSource, type RawCollectionItem } from "./lib/catalogItem";

export type Opts = { client?: ShopClient; signal?: AbortSignal };
export type PageMeta = { freshness: IndexFreshness | null };
export type ItemsPage<T = CatalogItem> = { items: T[]; total: number; meta: PageMeta };

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

function c(opts?: Opts): ShopClient {
  return opts?.client ?? shopClient();
}

function o(opts?: Opts) {
  return opts?.signal ? { signal: opts.signal } : undefined;
}

type DataTotal<T> = { data?: T[]; total?: number };

export type ShopSort = "newest" | "cheapest" | "most_expensive" | "name" | "discount" | "relevance";

export type ShopListingRaw = {
  tradeId: string | null;
  listingType: "primary" | "secondary";
  contractAddress: string;
  itemId: string | null;
  tokenId: string | null;
  name: string;
  thumbnail: string;
  rarity: string;
  category: string;
  wearableCategory: string | null;
  gender?: "male" | "female" | "unisex" | null;
  creator: string;
  priceCredits: number;
  available: number;
  network: string;
  chainId: number;
  isSmart?: boolean;
  emoteLoop?: boolean | null;
  seller?: string | null;
  issuedId?: string | null;
  compareAtCredits?: number | null;
  saleEndsAt?: number | null;
  coupon?: ShopCoupon | null;
  saleUnitsLeft?: number | null;
};

export type UnifiedListingRaw = ShopListingRaw & { source: ListingSource; acquisition?: ListingAcquisition; manaWei?: string | null };
type ShopItemRaw = UnifiedListingRaw & { listingCount?: number };

export type UnifiedListing = CatalogItem & { source: ListingSource; acquisition: ListingAcquisition; manaWei: string | null };

export type ShopListingFilters = {
  category?: string;
  first?: number;
  skip?: number;
  contractAddress?: string;
  contractAddresses?: string[];
  itemIds?: string[];
  itemId?: string;
  creator?: string;
  rarities?: string[];
  wearableCategories?: string[];
  wearableGenders?: string[];
  minPriceCredits?: number;
  maxPriceCredits?: number;
  search?: string;
  sortBy?: ShopSort;
  isSmart?: boolean;
  onSale?: boolean;
  discounted?: boolean;
  listingType?: "primary" | "secondary";
};

function listingRowId(l: ShopListingRaw): string {
  if (l.tradeId) return l.tradeId;
  const suffix = l.tokenId ? `t${l.tokenId}` : (l.itemId ?? "");
  return `${(l.contractAddress ?? "").toLowerCase()}-${suffix}`;
}

export function withoutSale(l: ShopListingRaw): ShopListingRaw {
  if (l.compareAtCredits == null) return { ...l, saleUnitsLeft: null, coupon: null };
  return { ...l, priceCredits: l.compareAtCredits, compareAtCredits: null, saleEndsAt: null, saleUnitsLeft: null, coupon: null };
}

export function shopListingToItem(raw: ShopListingRaw): CatalogItem {
  const l = shopFlags().creatorSales ? raw : withoutSale(raw);
  return {
    id: listingRowId(l),
    tradeId: l.tradeId ?? undefined,
    listingType: l.listingType,
    name: l.name,
    creator: l.creator,
    contractAddress: l.contractAddress,
    itemId: l.itemId,
    tokenId: l.tokenId ?? undefined,
    category: l.category,
    wearableCategory: l.wearableCategory ?? undefined,
    rarity: l.rarity,
    network: l.network,
    chainId: l.chainId,
    thumbnail: l.thumbnail,
    priceCredits: l.priceCredits,
    gender: l.gender ?? null,
    isSmart: l.isSmart ?? false,
    emoteLoop: l.emoteLoop,
    available: l.listingType === "primary" ? l.available : undefined,
    seller: l.seller ?? undefined,
    issuedId: l.issuedId ?? undefined,
    compareAtCredits: l.compareAtCredits != null && l.compareAtCredits > l.priceCredits ? l.compareAtCredits : undefined,
    saleEndsAt: l.saleEndsAt != null ? l.saleEndsAt * 1000 : undefined,
    coupon: l.coupon ?? undefined,
    saleUnitsLeft: l.saleUnitsLeft ?? undefined,
    onSale: true,
  };
}

export function unifiedListingToItem(l: UnifiedListingRaw): UnifiedListing {
  return { ...shopListingToItem(l), source: l.source, acquisition: l.acquisition ?? "trade", manaWei: l.manaWei ?? null };
}

function shopItemToItem(l: ShopItemRaw): UnifiedListing {
  return { ...unifiedListingToItem(l), listingCount: l.listingCount };
}

function shopQuery(params: ShopListingFilters): QueryParams {
  const q: QueryParams = {};
  if (params.category === "wearable" || params.category === "emote") q.category = params.category;
  if (params.first != null) q.first = params.first;
  if (params.skip != null) q.skip = params.skip;
  if (params.contractAddresses) q.contractAddress = params.contractAddresses.length ? params.contractAddresses.join(",") : ZERO_ADDRESS;
  else if (params.contractAddress) q.contractAddress = params.contractAddress;
  if (params.itemIds?.length) q.items = params.itemIds.join(",");
  if (params.itemId != null) q.itemId = params.itemId;
  if (params.creator) q.creator = params.creator;
  if (params.rarities?.length) q.rarity = params.rarities.join(",");
  if (params.wearableCategories?.length) q.wearableCategory = params.wearableCategories.join(",");
  if (params.wearableGenders?.length) q.wearableGender = params.wearableGenders.join(",");
  if (params.minPriceCredits != null) q.minPriceCredits = params.minPriceCredits;
  if (params.maxPriceCredits != null) q.maxPriceCredits = params.maxPriceCredits;
  if (params.search) q.search = params.search;
  if (params.sortBy) q.sortBy = params.sortBy;
  if (params.isSmart) q.isSmart = "true";
  if (params.onSale != null) q.onSale = String(params.onSale);
  if (params.discounted != null) q.discounted = String(params.discounted);
  if (params.listingType) q.listingType = params.listingType;
  return q;
}

export async function fetchShopListingsRaw(
  params: ShopListingFilters,
  opts?: Opts,
): Promise<{ listings: ShopListingRaw[]; total: number; creatorSalesLive: boolean; meta: PageMeta }> {
  const json = await c(opts).catalog.shop<DataTotal<ShopListingRaw>>(shopQuery(params), o(opts));
  return { listings: json.data ?? [], total: json.total ?? 0, creatorSalesLive: shopFlags().creatorSales, meta: { freshness: readFreshness(json) } };
}

type PeggedPrice = { priceCredits: number; tradeId?: string; compareAtCredits?: number; saleEndsAt?: number };

async function peggedPrices(scope: { contractAddress: string } | { creator: string }, keyOf: (l: ShopListingRaw) => string, opts?: Opts) {
  const map: Record<string, PeggedPrice> = {};
  const PAGE = 200;
  for (let skip = 0; ; skip += PAGE) {
    const { listings, total, creatorSalesLive } = await fetchShopListingsRaw({ ...scope, first: PAGE, skip, listingType: "primary" }, opts);
    for (const l of listings) {
      if (l.listingType !== "primary" || l.itemId == null) continue;
      map[keyOf(l)] = {
        priceCredits: l.priceCredits,
        ...(l.tradeId ? { tradeId: l.tradeId } : {}),
        ...(creatorSalesLive && l.compareAtCredits != null ? { compareAtCredits: l.compareAtCredits } : {}),
        ...(creatorSalesLive && l.saleEndsAt != null ? { saleEndsAt: l.saleEndsAt * 1000 } : {}),
      };
    }
    if (listings.length < PAGE || (total > 0 && skip + listings.length >= total)) break;
  }
  return map;
}

export function fetchPeggedPrimaryPrices(contractAddress: string, opts?: Opts): Promise<Record<string, PeggedPrice>> {
  return peggedPrices({ contractAddress }, (l) => String(l.itemId), opts);
}

export function fetchCreatorPeggedPrimaryPrices(creator: string, opts?: Opts): Promise<Record<string, PeggedPrice>> {
  return peggedPrices({ creator: creator.toLowerCase() }, (l) => `${l.contractAddress.toLowerCase()}-${l.itemId}`, opts);
}

export async function fetchListings({ first = 100, ...filters }: ShopListingFilters = {}, opts?: Opts): Promise<ItemsPage> {
  const { listings, total, meta } = await fetchShopListingsRaw({ ...filters, first }, opts);
  return { items: listings.map(shopListingToItem), total, meta };
}

function unifiedQuery(first: number, filters: ShopListingFilters, groupBy?: "item"): QueryParams {
  return { ...shopQuery({ ...filters, first }), ...(groupBy ? { groupBy } : {}) };
}

export async function fetchUnified({ first = 100, ...filters }: ShopListingFilters = {}, opts?: Opts): Promise<ItemsPage<UnifiedListing>> {
  const json = await c(opts).catalog.unified<DataTotal<UnifiedListingRaw>>(unifiedQuery(first, filters), o(opts));
  const data = json.data ?? [];
  return { items: data.map(unifiedListingToItem), total: json.total ?? data.length, meta: { freshness: readFreshness(json) } };
}

export async function fetchShopItems({ first = 100, ...filters }: ShopListingFilters = {}, opts?: Opts): Promise<ItemsPage<UnifiedListing>> {
  const json = await c(opts).catalog.unified<DataTotal<ShopItemRaw>>(unifiedQuery(first, filters, "item"), o(opts));
  const data = json.data ?? [];
  return { items: data.map(shopItemToItem), total: json.total ?? data.length, meta: { freshness: readFreshness(json) } };
}

export async function fetchRelatedItems(
  contractAddress: string,
  itemId: string,
  { first = 10, listingType }: { first?: number; listingType?: "primary" | "secondary" } = {},
  opts?: Opts,
): Promise<UnifiedListing[]> {
  const json = await c(opts).catalog.related<{ data?: ShopItemRaw[] }>({ contractAddress, itemId, first, listingType }, o(opts));
  return (json.data ?? []).map(shopItemToItem);
}

export async function fetchTrendingItems(
  { first = 12, listingType }: { first?: number; listingType?: "primary" | "secondary" } = {},
  opts?: Opts,
): Promise<UnifiedListing[]> {
  const json = await c(opts).catalog.trending<{ data?: ShopItemRaw[] }>({ first, includeSocialEmotes: "false", listingType }, o(opts));
  return (json.data ?? []).map(shopItemToItem);
}

export type SuggestedItem = UnifiedListing & { reason: SuggestionReason; score: number };
export type SuggestedItemsResult = { data: SuggestedItem[]; personalized: boolean; algorithm: string };

const EQUIPPED_URN = /^urn:decentraland:(?:matic|amoy):collections-v2:(0x[0-9a-f]{40}):(\d+)$/i;

export function compactEquipped(urns: string[]): string[] {
  const ids: string[] = [];
  for (const urn of urns) {
    const match = EQUIPPED_URN.exec(urn.trim());
    if (match) ids.push(`${match[1]!.toLowerCase()}-${match[2]}`);
  }
  return ids;
}

export async function fetchSuggestedItems(
  {
    address,
    seeds,
    bodyShape,
    equipped,
    exclude,
    category,
    first = 12,
  }: { address?: string; seeds?: string[]; bodyShape?: string; equipped?: string[]; exclude?: string[]; category?: string; first?: number } = {},
  opts?: Opts,
): Promise<SuggestedItemsResult> {
  const q: QueryParams = { first };
  if (address) q.address = address;
  if (seeds?.length) q.seeds = seeds.join(",");
  if (bodyShape) q.bodyShape = bodyShape;
  if (equipped?.length) q.equipped = compactEquipped(equipped).join(",");
  if (exclude?.length) q.exclude = exclude.join(",");
  if (category) q.category = category;
  const json = await c(opts).catalog.suggested<{
    data?: Array<ShopItemRaw & { reason?: SuggestionReason; score?: number }>;
    personalized?: boolean;
    algorithm?: string;
  }>(q, o(opts));
  return {
    data: (json.data ?? []).map((row) => ({ ...shopItemToItem(row), reason: row.reason ?? { kind: "trending" }, score: row.score ?? 0 })),
    personalized: json.personalized === true,
    algorithm: json.algorithm ?? "unknown",
  };
}

export async function fetchCatalogByIds(ids: string[], opts?: Opts): Promise<CatalogItem[]> {
  if (ids.length === 0) return [];
  const CHUNK = 50;
  const chunks: string[][] = [];
  for (let i = 0; i < ids.length; i += CHUNK) chunks.push(ids.slice(i, i + CHUNK));
  const pages = await Promise.all(
    chunks.map(async (chunk) => {
      const json = await c(opts).catalog.items<DataTotal<RawCollectionItem>>({ first: chunk.length, id: chunk }, o(opts));
      return (json.data ?? []).map(toCatalogItem);
    }),
  );
  const byId = new Map(pages.flat().map((item) => [item.id, item]));
  return ids.map((id) => byId.get(id)).filter((item): item is CatalogItem => !!item);
}

export type CatalogItemsFilters = {
  first?: number;
  skip?: number;
  category?: string;
  creator?: string;
  contractAddress?: string;
  contractAddresses?: string[];
  rarities?: string[];
  wearableCategories?: string[];
  wearableGenders?: string[];
  search?: string;
  sortBy?: string;
  isWearableSmart?: boolean;
  isOnSale?: boolean;
  minPriceCredits?: number;
  maxPriceCredits?: number;
};

async function withRunningSales(items: CatalogItem[], scope: { creator?: string; contractAddress?: string }, opts?: Opts): Promise<CatalogItem[]> {
  if (!scope.creator && !scope.contractAddress) return items;
  if (!items.some((i) => i.itemId != null)) return items;
  try {
    const { listings, creatorSalesLive } = await fetchShopListingsRaw({ ...scope, first: 200, listingType: "primary" }, opts);
    if (!creatorSalesLive) return items;
    const sales = new Map<string, { compareAtCredits: number; saleEndsAt?: number; priceCredits: number }>();
    for (const l of listings) {
      if (l.itemId == null || l.compareAtCredits == null) continue;
      sales.set(`${l.contractAddress.toLowerCase()}-${l.itemId}`, {
        compareAtCredits: l.compareAtCredits,
        priceCredits: l.priceCredits,
        ...(l.saleEndsAt != null ? { saleEndsAt: l.saleEndsAt * 1000 } : {}),
      });
    }
    if (sales.size === 0) return items;
    return items.map((item) => {
      const sale = item.itemId == null ? undefined : sales.get(`${item.contractAddress.toLowerCase()}-${item.itemId}`);
      return sale ? { ...item, ...sale } : item;
    });
  } catch {
    return items;
  }
}

export async function fetchCatalogItems(
  {
    first = 48,
    skip = 0,
    category,
    creator,
    contractAddress,
    contractAddresses,
    rarities,
    wearableCategories,
    wearableGenders,
    search,
    sortBy,
    isWearableSmart,
    isOnSale,
    minPriceCredits,
    maxPriceCredits,
  }: CatalogItemsFilters = {},
  opts?: Opts,
): Promise<ItemsPage> {
  const q: QueryParams = { first, skip, includeSocialEmotes: "false" };
  if (category === "wearable" || category === "emote") q.category = category;
  if (creator) q.creator = creator;
  if (contractAddresses) q.contractAddress = contractAddresses.length ? contractAddresses : [ZERO_ADDRESS];
  else if (contractAddress) q.contractAddress = contractAddress;
  if (rarities?.length) q.rarity = rarities;
  if (wearableCategories?.length) q.wearableCategory = wearableCategories;
  if (wearableGenders?.length) q.wearableGender = wearableGenders;
  if (search) q.search = search;
  if (sortBy) q.sortBy = sortBy;
  if (isWearableSmart) q.isWearableSmart = "true";
  if (isOnSale != null) q.isOnSale = String(isOnSale);
  if (minPriceCredits != null) q.minPriceCredits = minPriceCredits;
  if (maxPriceCredits != null) q.maxPriceCredits = maxPriceCredits;
  const json = await c(opts).catalog.items<DataTotal<RawCollectionItem>>(q, o(opts));
  const items = await withRunningSales((json.data ?? []).map(toCatalogItem), { creator, contractAddress }, opts);
  return { items, total: json.total ?? skip + items.length, meta: { freshness: readFreshness(json) } };
}

export type CollectionItemsFilters = {
  category?: string;
  rarities?: string[];
  wearableCategories?: string[];
  minPriceCredits?: number;
  maxPriceCredits?: number;
  sortBy?: string;
};

export async function fetchCollectionItems(
  contractAddress: string,
  { first = 20, skip = 0, ...filters }: { first?: number; skip?: number } & CollectionItemsFilters = {},
  opts?: Opts,
): Promise<ItemsPage> {
  return fetchCatalogItems(
    {
      contractAddress,
      first,
      skip,
      category: filters.category && filters.category !== "all" ? filters.category : undefined,
      rarities: filters.rarities,
      wearableCategories: filters.wearableCategories,
      minPriceCredits: filters.minPriceCredits,
      maxPriceCredits: filters.maxPriceCredits,
      sortBy: filters.sortBy,
    },
    opts,
  );
}

type RawCollection = { contractAddress: string; name?: string; creator?: string; size?: number; isOnSale?: boolean; chainId?: number; network?: string };

export type CollectionMeta = { contractAddress: string; name: string; creator: string; chainId?: number; network?: string };
export type CollectionSummary = CollectionMeta & { itemCount: number; isOnSale?: boolean };

export function sanitizeCollectionName(name: string): string {
  if (!name.startsWith("dcl://")) return name;
  return name
    .slice("dcl://".length)
    .split("_")
    .map((word) => (word ? word.charAt(0).toUpperCase() + word.slice(1) : ""))
    .join(" ");
}

function toCollection(c: RawCollection): CollectionSummary {
  return {
    contractAddress: c.contractAddress,
    name: sanitizeCollectionName(c.name ?? ""),
    creator: c.creator ?? "",
    itemCount: c.size ?? 0,
    isOnSale: c.isOnSale,
    chainId: c.chainId,
    network: c.network,
  };
}

export async function fetchCreatorCollections(
  creator: string,
  { first = 24, skip = 0 }: { first?: number; skip?: number } = {},
  opts?: Opts,
): Promise<{ collections: CollectionSummary[]; total: number }> {
  const json = await c(opts).market.collections<DataTotal<RawCollection>>({ creator, first, skip, sortBy: "newest" }, o(opts));
  const collections = (json.data ?? []).map(toCollection);
  return { collections, total: json.total ?? skip + collections.length };
}

export async function fetchCollection(contractAddress: string, opts?: Opts): Promise<CollectionMeta | null> {
  const json = await c(opts).market.collections<DataTotal<RawCollection>>({ contractAddress, first: 1 }, o(opts));
  const row = json.data?.[0];
  return row && row.contractAddress ? toCollection(row) : null;
}

export type ShopCreatorRank = { id: string; volumeWei?: string; sales: number; totalSales?: number; collections?: number; items?: number };

export async function fetchShopTopCreators(first: number, days: number, opts?: Opts): Promise<ShopCreatorRank[]> {
  const json = await c(opts).catalog.creators<{ data?: ShopCreatorRank[] }>({ first, days }, o(opts));
  return json.data ?? [];
}

export type SuggestedSearchItem = CatalogItem & { creatorName: string | null };
export type CollectionHit = { contractAddress: string; name: string; creator: string; creatorName: string | null; items: number; sales: number };
export type CreatorHit = { address: string; name: string; face?: string };
export type Suggestions = { items: SuggestedSearchItem[]; total: number; collections: CollectionHit[]; creators: CreatorHit[] };
export type SuggestionSizes = { items?: number; collections?: number; creators?: number };

export const EMPTY_SUGGESTIONS: Suggestions = { items: [], total: 0, collections: [], creators: [] };

type RawSuggestions = {
  items?: { data?: (RawCollectionItem & { creatorName?: string | null })[]; total?: number };
  collections?: { data?: Partial<CollectionHit>[] };
  creators?: { data?: { address?: string; name?: string; face?: string | null }[] };
};

export async function fetchSuggestions(search: string, { items = 5, collections = 4, creators = 4 }: SuggestionSizes = {}, opts?: Opts): Promise<Suggestions> {
  const term = search.trim();
  if (!term) return EMPTY_SUGGESTIONS;
  const body = await c(opts).catalog.suggest<RawSuggestions>({ search: term, items, collections, creators }, o(opts));
  return {
    items: (body.items?.data ?? []).map((row) => ({ ...toCatalogItem(row), creatorName: row.creatorName ?? null })),
    total: body.items?.total ?? 0,
    collections: (body.collections?.data ?? [])
      .filter((x) => x.contractAddress && x.name)
      .map((x) => ({
        contractAddress: x.contractAddress!,
        name: x.name!,
        creator: x.creator ?? "",
        creatorName: x.creatorName ?? null,
        items: x.items ?? 0,
        sales: x.sales ?? 0,
      })),
    creators: (body.creators?.data ?? [])
      .filter((x) => x.address && x.name)
      .map((x) => ({ address: x.address!.toLowerCase(), name: x.name!, face: x.face ?? undefined })),
  };
}

export async function searchCreators(search: string, first = 10, opts?: Opts): Promise<CreatorHit[]> {
  const term = search.trim();
  if (!term) return [];
  const json = await c(opts).catalog.searchCreators<{ data?: { address?: string; name?: string; face?: string | null }[] } | Array<{ address?: string; name?: string }>>(
    { search: term, first },
    o(opts),
  );
  const rows = Array.isArray(json) ? json : (json.data ?? []);
  return rows
    .filter((x) => x.address && x.name)
    .map((x) => ({ address: x.address!.toLowerCase(), name: x.name!, face: ("face" in x ? (x.face ?? undefined) : undefined) as string | undefined }));
}
