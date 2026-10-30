// from: shop@94584a5c:app/src/lib/api.ts (fetchMyAssets, fetchOwnedToken, fetchUserSales, toMyAsset);
//       marketplace@f3956826:webapp/src/modules/vendor/decentraland/{account,analytics,bid,marketplace,rankings,sale,order}/api.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
//
// The seller and marketplace reads, mapped from the wire onto what the pages render, over the ONE client
// (api/shop.ts). One deliberate change from upstream's `toMyAsset`: an owned token's open listing is a
// RESALE, so its price is kept as the MANA (or USD-pegged) amount the order signed, never converted into
// a credit price. A 503 unavailable envelope (a route whose indexer has not caught up) is surfaced as
// `DataUnavailable`, with the missing source, never as an empty page.

import type { Account } from "@catalyst-api/market/Account";
import type { Bid } from "@catalyst-api/market/Bid";
import type { Collection } from "@catalyst-api/market/Collection";
import type { NftResult } from "@catalyst-api/market/NftResult";
import type { Order } from "@catalyst-api/market/Order";
import type { Owner } from "@catalyst-api/market/Owner";
import type { Sale } from "@catalyst-api/market/Sale";
import { isShopApiError } from "../../api/client";
import { shopClient, type ShopClient } from "../../api/shop";
import { parseManaWei, type ManaWei } from "../../model/money";

export type { Account, Bid, Collection, Order, Owner, Sale };

const client = (c?: ShopClient) => c ?? shopClient();

/** What a 503 unavailable envelope says is missing (`src/marketplace/mod.rs` `unavailable`). */
export type DataUnavailable = { source: string; message: string; configure: string | null };

export function dataUnavailable(e: unknown): DataUnavailable | null {
  if (!isShopApiError(e) || e.status !== 503) return null;
  const body = (e.body ?? {}) as { source?: unknown; message?: unknown; configure?: unknown };
  return {
    source: typeof body.source === "string" ? body.source : "unknown",
    message: typeof body.message === "string" ? body.message : e.message,
    configure: typeof body.configure === "string" ? body.configure : null,
  };
}

/** The price an open order carries: MANA wei for a MANA order, USD wei for a USD-pegged one. */
export type ListingPrice = { kind: "mana"; wei: ManaWei } | { kind: "usdPegged"; usdWei: bigint };

/** One owned token, as My Items and the owner's manage view render it. */
export type MyAsset = {
  id: string;
  contractAddress: string;
  tokenId: string;
  issuedId?: string;
  itemId: string | null;
  name: string;
  category: string;
  image: string;
  rarity?: string;
  network: string;
  chainId: number;
  urn?: string;
  isOnSale: boolean;
  /** The open listing's signed price; a resale, so never a credit price. */
  listing?: { tradeId: string | null; orderId: string; price: ListingPrice | null };
  /** A LAND/estate rented out through the rentals contract. */
  openRentalId: string | null;
};

function rarityOf(data: NftResult["nft"]["data"]): string | undefined {
  if ("wearable" in data) return data.wearable.rarity;
  if ("emote" in data) return data.emote.rarity;
  return undefined;
}

/** A price as the order stores it: a trade's USD-pegged amount is marked `usd:` by the feed only through its trade. */
function orderPrice(order: Order): ListingPrice | null {
  const wei = parseManaWei(order.price);
  return wei === null || wei === 0n ? null : { kind: "mana", wei };
}

/** Upstream `toMyAsset`, with the listing price kept in its signed currency. */
export function toMyAsset(r: NftResult): MyAsset {
  return {
    id: r.nft.id,
    contractAddress: r.nft.contractAddress.toLowerCase(),
    tokenId: r.nft.tokenId,
    issuedId: r.nft.issuedId ?? undefined,
    itemId: r.nft.itemId ?? null,
    name: r.nft.name,
    category: r.nft.category,
    image: r.nft.image,
    rarity: rarityOf(r.nft.data),
    network: r.nft.network,
    chainId: r.nft.chainId,
    urn: r.nft.urn,
    isOnSale: r.order != null,
    listing: r.order ? { tradeId: r.order.tradeId, orderId: r.order.id, price: orderPrice(r.order) } : undefined,
    openRentalId: r.nft.openRentalId,
  };
}

export type MyAssetsSort = "newest" | "name" | "cheapest";
export type MyAssetsCategory = "wearable" | "emote" | "ens" | "parcel" | "estate";

export type MyAssetsFilters = {
  category?: MyAssetsCategory;
  first?: number;
  skip?: number;
  search?: string;
  rarities?: string[];
  wearableCategories?: string[];
  emoteCategories?: string[];
  onlyOnSale?: boolean;
  onlyOnRent?: boolean;
  sortBy?: MyAssetsSort;
};

/** Upstream `fetchMyAssets`: an account's owned NFTs from `/v1/nfts`, filtered and sorted server-side. */
export async function fetchMyAssets(owner: string, filters: MyAssetsFilters = {}, c?: ShopClient): Promise<{ assets: MyAsset[]; total: number }> {
  const res = await client(c).market.nfts({
    owner: owner.toLowerCase(),
    category: filters.category ?? "wearable",
    first: filters.first ?? 48,
    skip: filters.skip ?? 0,
    sortBy: filters.sortBy ?? "newest",
    orderDirection: "desc",
    search: filters.search,
    itemRarity: filters.rarities,
    wearableCategory: filters.wearableCategories,
    emoteCategory: filters.emoteCategories,
    isOnSale: filters.onlyOnSale ? "true" : undefined,
    isOnRent: filters.onlyOnRent ? "true" : undefined,
  });
  return { assets: res.data.map(toMyAsset), total: res.total };
}

/** Upstream `fetchOwnedToken`: `owner`'s token, or null when they do not hold it (or on a failed read). */
export async function fetchOwnedToken(owner: string, contractAddress: string, tokenId: string, c?: ShopClient): Promise<MyAsset | null> {
  try {
    const res = await client(c).market.nfts({ owner: owner.toLowerCase(), contractAddress: contractAddress.toLowerCase(), tokenId, first: 1 });
    const row = res.data[0];
    if (!row || row.nft.tokenId !== tokenId || row.nft.owner.toLowerCase() !== owner.toLowerCase()) return null;
    return toMyAsset(row);
  } catch {
    return null;
  }
}

/** Any token by id, whoever holds it (the public token page, a bid target). Null when unknown. */
export async function fetchToken(contractAddress: string, tokenId: string, c?: ShopClient): Promise<(MyAsset & { owner: string }) | null> {
  const res = await client(c).market.nfts({ contractAddress: contractAddress.toLowerCase(), tokenId, first: 1 });
  const row = res.data[0];
  if (!row || row.nft.tokenId !== tokenId) return null;
  return { ...toMyAsset(row), owner: row.nft.owner.toLowerCase() };
}

/** Upstream `SaleRecord`: one completed sale, prices in MANA wei. */
export type SaleRecord = {
  id: string;
  buyer: string;
  seller: string;
  contractAddress: string;
  tokenId: string | null;
  itemId: string | null;
  manaWei: ManaWei;
  createdAt: number;
  txHash: string;
  type: string;
  chainId: number;
};

export function toSaleRecord(s: Sale): SaleRecord {
  return {
    id: s.id,
    buyer: s.buyer,
    seller: s.seller,
    contractAddress: s.contractAddress,
    tokenId: s.tokenId,
    itemId: s.itemId,
    manaWei: parseManaWei(s.price) ?? (0n as ManaWei),
    createdAt: s.timestamp,
    txHash: s.txHash,
    type: s.type,
    chainId: s.chainId,
  };
}

/** Upstream `fetchUserSales`: an account's sales as seller or buyer, newest first. */
export async function fetchUserSales(
  address: string,
  opts: { role?: "seller" | "buyer"; first?: number; skip?: number } = {},
  c?: ShopClient,
): Promise<{ items: SaleRecord[]; total: number }> {
  const role = opts.role ?? "seller";
  const res = await client(c).market.sales({ [role]: address.toLowerCase(), first: opts.first ?? 24, skip: opts.skip ?? 0, sortBy: "recently_sold" });
  return { items: res.data.map(toSaleRecord), total: res.total };
}

/** marketplace FETCH_SALES: sales of an asset, a contract or an account. */
export async function fetchSales(
  filters: { contractAddress?: string; tokenId?: string; itemId?: string; seller?: string; buyer?: string; first?: number; skip?: number },
  c?: ShopClient,
): Promise<{ items: SaleRecord[]; total: number }> {
  const res = await client(c).market.sales({ ...filters, sortBy: "recently_sold" });
  return { items: res.data.map(toSaleRecord), total: res.total };
}

/** marketplace FETCH_ACCOUNT_METRICS: sales, purchases, spent, earned and royalties of accounts. */
export async function fetchAccountMetrics(addresses: string[], opts: { network?: "ETHEREUM" | "MATIC" } = {}, c?: ShopClient): Promise<Account[]> {
  const res = await client(c).market.accounts({ address: addresses.map((a) => a.toLowerCase()), network: opts.network });
  return res.data;
}

/** marketplace account `GET /collections`: a creator's published collections. */
export async function fetchAccountCollections(creator: string, opts: { first?: number; skip?: number } = {}, c?: ShopClient): Promise<{ data: Collection[]; total: number }> {
  return client(c).market.collections({ creator: creator.toLowerCase(), first: opts.first ?? 24, skip: opts.skip ?? 0, sortBy: "newest" });
}

export type AnalyticsTimeframe = "day" | "week" | "month" | "all";

/** v2's `VolumeData` (src/marketplace/rankings.rs): sums as MANA wei decimal strings. */
export type VolumeData = { sales: number; volume: string; creatorsEarnings: string; daoEarnings: string };

/** marketplace FETCH_ANALYTICS_VOLUME_DATA. */
export async function fetchVolume(timeframe: AnalyticsTimeframe, c?: ShopClient): Promise<VolumeData> {
  const res = await client(c).market.volume<{ data: VolumeData }>(timeframe);
  return res.data;
}

export type RankingEntity = "wearables" | "emotes" | "creators" | "collectors";
export type RankingsSortBy = "most_volume" | "most_sales";
export type ItemRank = { id: string; sales: number; volume: string };
export type CreatorRank = { id: string; sales: number; earned: string; collections: number; uniqueCollectors: number };
export type CollectorRank = { id: string; purchases: number; spent: string; uniqueAndMythicItems: number; creatorsSupported: number };
export type RankRow = ItemRank | CreatorRank | CollectorRank;

/** marketplace FETCH_RANKINGS: the top entities of a timeframe (items ids are `<contract>-<itemId>`). */
export async function fetchRankings(
  entity: RankingEntity,
  timeframe: AnalyticsTimeframe,
  filters: { category?: string; rarity?: string; sortBy?: RankingsSortBy; first?: number } = {},
  c?: ShopClient,
): Promise<RankRow[]> {
  const res = await client(c).market.rankings<{ data: RankRow[] }>(entity, timeframe, { ...filters, first: filters.first ?? 5 });
  return res.data;
}

export type BidStatus = "open" | "sold" | "cancelled";

function bidsPage(body: unknown): { results: Bid[]; total: number } {
  const b = body as { ok?: boolean; data?: { results: Bid[]; total: number }; results?: Bid[]; total?: number };
  if (b.data) return b.data;
  return { results: b.results ?? [], total: b.total ?? 0 };
}

/** marketplace FETCH_BIDS_BY_ADDRESS: open bids an address received (as the owner) and made (as bidder). */
export async function fetchBidsByAddress(address: string, c?: ShopClient): Promise<{ received: Bid[]; made: Bid[] }> {
  const owner = address.toLowerCase();
  const [received, made] = await Promise.all([
    client(c).market.bids({ seller: owner, status: "open", limit: 1000 }),
    client(c).market.bids({ bidder: owner, status: "open", limit: 1000 }),
  ]);
  return { received: bidsPage(received).results, made: bidsPage(made).results };
}

/** marketplace FETCH_BIDS_BY_ASSET: open bids on a token, or on any token of an item. */
export async function fetchBidsByAsset(
  asset: { contractAddress: string } & ({ tokenId: string } | { itemId: string }),
  opts: { status?: BidStatus; sortBy?: string; first?: number; skip?: number } = {},
  c?: ShopClient,
): Promise<{ results: Bid[]; total: number }> {
  const target = "tokenId" in asset ? { tokenId: asset.tokenId } : { itemId: asset.itemId };
  return bidsPage(
    await client(c).market.bids({
      contractAddress: asset.contractAddress.toLowerCase(),
      ...target,
      status: opts.status ?? "open",
      sortBy: opts.sortBy,
      limit: opts.first ?? 1000,
      offset: opts.skip ?? 0,
    }),
  );
}

/** marketplace FETCH_LEGACY_ORDERS: on-chain (pre-trades) orders of a token. */
export async function fetchLegacyOrders(filters: { contractAddress: string; tokenId?: string; owner?: string; status?: "open" | "sold" | "cancelled" }, c?: ShopClient): Promise<Order[]> {
  const res = await client(c).market.orders({ ...filters, contractAddress: filters.contractAddress.toLowerCase(), status: filters.status ?? "open" });
  return res.data;
}

/** An open order of `owner` by its id (an old link naming only `?order=`), or null when it is not open any more. */
export async function fetchOpenOrderById(owner: string, id: string, c?: ShopClient): Promise<Order | null> {
  const res = await client(c).market.orders({ owner: owner.toLowerCase(), status: "open", first: 1000 });
  return res.data.find((o) => o.id === id && !!o.tokenId) ?? null;
}

/** marketplace `GET /v1/owners`: who holds the tokens of an item. */
export async function fetchOwners(
  filters: { contractAddress: string; itemId: string; first?: number; skip?: number; sortBy?: "issuedId"; orderDirection?: "asc" | "desc" },
  c?: ShopClient,
): Promise<{ data: Owner[]; total: number }> {
  return client(c).market.owners({ ...filters, contractAddress: filters.contractAddress.toLowerCase() });
}

/** marketplace `GET /v1/stats/estate/size`: estate counts by size (deferred on v2 until its indexer runs). */
export async function fetchEstateSizes(filters: Record<string, string | number | undefined> = {}, c?: ShopClient): Promise<Record<string, number>> {
  return client(c).market.stats<Record<string, number>>("estate", "size", filters);
}
