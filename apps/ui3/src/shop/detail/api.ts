// from: shop@94584a5c:app/src/lib/api.ts (the item and token reads of the detail page);
//       marketplace@f3956826:webapp/src/modules/vendor/decentraland/nft/{api,authApi}.ts, webapp/src/modules/{item,nft}/actions.ts (FETCH_ITEM_REQUEST, FETCH_NFT_REQUEST)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import type { NftResult } from "@catalyst-api/market/NftResult";
import type { Order } from "@catalyst-api/market/Order";
import { fetchUnified, fetchShopListingsRaw, shopListingToItem, type Opts, type UnifiedListing } from "../discovery/api";
import { toGender, type CatalogItem, type Gender, type RawCollectionItem } from "../discovery/lib/catalogItem";
import { shopClient, type ShopClient } from "../api/shop";

function c(opts?: Opts): ShopClient {
  return opts?.client ?? shopClient();
}

function o(opts?: Opts) {
  return opts?.signal ? { signal: opts.signal } : undefined;
}

type DataTotal<T> = { data?: T[]; total?: number };

export async function fetchItemDescription(contractAddress: string, itemId: string, opts?: Opts): Promise<string> {
  try {
    const json = await c(opts).catalog.v2<DataTotal<RawCollectionItem>>({ id: `${contractAddress.toLowerCase()}-${itemId}`, first: 1 }, o(opts));
    const d = json.data?.[0]?.data;
    return (d?.wearable?.description ?? d?.emote?.description ?? "").trim();
  } catch {
    return "";
  }
}

export function pickItemListing<T extends Pick<UnifiedListing, "tokenId" | "source">>(items: T[]): T | null {
  const rank = (l: T) => (l.tokenId ? 2 : 0) + (l.source === "native" ? 0 : 1);
  return [...items].sort((a, b) => rank(a) - rank(b))[0] ?? null;
}

export async function fetchUnifiedListingForItem(contractAddress: string, itemId: string, opts?: Opts): Promise<UnifiedListing | null> {
  const { items } = await fetchUnified({ contractAddress, itemId, first: 5 }, opts);
  return pickItemListing(items);
}

export async function fetchPrimaryListingForItem(contractAddress: string, itemId: string, opts?: Opts): Promise<UnifiedListing | null> {
  const { items } = await fetchUnified({ contractAddress, itemId, first: 5, listingType: "primary" }, opts);
  return items.find((l) => !l.tokenId) ?? null;
}

export async function fetchShopListingForItem(contractAddress: string, itemId: string, opts?: Opts): Promise<CatalogItem | null> {
  const { listings } = await fetchShopListingsRaw({ contractAddress, itemId, first: 1 }, opts);
  return listings[0] ? shopListingToItem(listings[0]) : null;
}

export async function fetchStoreMintState(contractAddress: string, itemId: string, opts?: Opts): Promise<{ priceWei: string; available: number } | null> {
  const { items } = await fetchUnified({ contractAddress, itemId, first: 1 }, opts);
  const row = items[0];
  if (!row || row.acquisition !== "store" || !row.manaWei) return null;
  return { priceWei: row.manaWei, available: row.available ?? 0 };
}

export async function fetchItemResales(contractAddress: string, itemId: string, opts?: Opts): Promise<UnifiedListing[]> {
  const { items } = await fetchUnified({ contractAddress, itemId, first: 100, sortBy: "cheapest" }, opts);
  return items.filter((i) => !!i.tokenId && !!i.tradeId).sort((a, b) => a.priceCredits - b.priceCredits);
}

export async function fetchSecondarySaleState(contractAddress: string, opts?: Opts): Promise<Record<string, { tradeId: string }>> {
  const { listings } = await fetchShopListingsRaw({ contractAddress, first: 200 }, opts);
  const map: Record<string, { tradeId: string }> = {};
  for (const l of listings) {
    if (l.listingType !== "secondary" || l.tokenId == null || !l.tradeId) continue;
    map[String(l.tokenId)] = { tradeId: l.tradeId };
  }
  return map;
}

export type ClassicOrder = { tokenId: string; issuedId?: string; seller: string; contractAddress: string; priceWei: string; expiresAt: number | null };

export async function fetchClassicItemOrders(contractAddress: string, itemId: string, opts?: Opts): Promise<ClassicOrder[]> {
  const json = await c(opts).market.orders<DataTotal<Order & { itemId?: string }>>(
    { contractAddress, itemId, status: "open", sortBy: "cheapest", first: 100 },
    o(opts),
  );
  return (json.data ?? [])
    .filter((row) => !row.tradeId && !!row.tokenId)
    .map((row) => ({
      tokenId: row.tokenId!,
      issuedId: row.issuedId ?? undefined,
      seller: row.owner,
      contractAddress: row.contractAddress,
      priceWei: row.price,
      expiresAt: row.expiresAt || null,
    }));
}

export type OwnedToken = {
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
  owner: string;
  urn?: string;
  isOnSale: boolean;
  orderPriceWei?: string;
  tradeId?: string;
  legacyOrder?: LegacyOrderRef;
};

export type LegacyOrderRef = { id: string; marketplaceAddress: string; price: string; chainId: number };

function nftRarity(data: NftResult["nft"]["data"]): string | undefined {
  const d = data as { wearable?: { rarity?: string }; emote?: { rarity?: string } } | undefined;
  return d?.wearable?.rarity ?? d?.emote?.rarity;
}

function toOwnedToken(r: NftResult): OwnedToken {
  return {
    id: r.nft.id,
    contractAddress: r.nft.contractAddress,
    tokenId: r.nft.tokenId,
    issuedId: r.nft.issuedId ?? undefined,
    itemId: r.nft.itemId ?? null,
    name: r.nft.name,
    category: r.nft.category,
    image: r.nft.image,
    rarity: nftRarity(r.nft.data),
    network: r.nft.network,
    chainId: r.nft.chainId,
    owner: r.nft.owner,
    urn: r.nft.urn,
    isOnSale: r.order != null,
    orderPriceWei: r.order?.price ?? undefined,
    tradeId: r.order?.tradeId ?? undefined,
    ...(r.order && !r.order.tradeId
      ? { legacyOrder: { id: r.order.id, marketplaceAddress: r.order.marketplaceAddress, price: r.order.price, chainId: r.order.chainId } }
      : {}),
  };
}

export async function fetchTokenById(contractAddress: string, tokenId: string, opts?: Opts): Promise<OwnedToken | null> {
  const json = await c(opts).market.nfts<DataTotal<NftResult>>({ contractAddress, tokenId, first: 1 }, o(opts));
  const r = json.data?.[0];
  return r && r.nft.tokenId === tokenId ? toOwnedToken(r) : null;
}

export async function fetchOwnedToken(owner: string, contractAddress: string, tokenId: string, opts?: Opts): Promise<OwnedToken | null> {
  try {
    const json = await c(opts).market.nfts<DataTotal<NftResult>>({ owner: owner.toLowerCase(), contractAddress, tokenId, first: 1 }, o(opts));
    const r = json.data?.[0];
    return r && r.nft.tokenId === tokenId ? toOwnedToken(r) : null;
  } catch {
    return null;
  }
}

export async function fetchOwnsItem(owner: string, contractAddress: string, itemId: string, opts?: Opts): Promise<boolean> {
  return (await fetchOwnedItemCount(owner, contractAddress, itemId, opts)) > 0;
}

export async function fetchOwnedItemCount(owner: string, contractAddress: string, itemId: string, opts?: Opts): Promise<number> {
  try {
    const json = await c(opts).market.nfts<DataTotal<NftResult>>({ owner: owner.toLowerCase(), contractAddress, itemId, first: 1 }, o(opts));
    return typeof json.total === "number" ? json.total : 0;
  } catch {
    return 0;
  }
}

export type ResaleTokenInfo = { seller?: string; issuedId?: string };

export async function fetchResaleTokenInfo(contractAddress: string, tokenId: string, opts?: Opts): Promise<ResaleTokenInfo> {
  try {
    const json = await c(opts).market.nfts<DataTotal<NftResult>>({ contractAddress, tokenId, first: 1 }, o(opts));
    const row = json.data?.[0];
    if (!row?.nft || row.nft.tokenId !== tokenId) return {};
    return { seller: row.nft.owner ?? row.order?.owner, issuedId: row.nft.issuedId ?? undefined };
  } catch {
    return {};
  }
}

export async function fetchResaleTokenInfos(contractAddress: string, tokenIds: string[], opts?: Opts): Promise<Record<string, ResaleTokenInfo>> {
  const entries = await Promise.all(tokenIds.map(async (id) => [id, await fetchResaleTokenInfo(contractAddress, id, opts)] as const));
  return Object.fromEntries(entries);
}

export type ItemMeta = {
  name?: string;
  thumbnail?: string;
  isSmart: boolean;
  utility: string | null;
  urn: string | null;
  wearableCategory: string | null;
  gender: Gender;
  available: number | null;
  creator: string | null;
  rarity: string | null;
  category: string | null;
  chainId: number | null;
  network: string | null;
  emoteLoop?: boolean;
  emoteHasSound?: boolean;
  emoteHasProps?: boolean;
};

function toSupply(raw: string | number | undefined | null): number | null {
  if (raw == null) return null;
  if (typeof raw === "string" && raw.trim() === "") return null;
  const n = typeof raw === "number" ? raw : Number(raw);
  return Number.isInteger(n) && n >= 0 ? n : null;
}

export async function fetchItemMeta(contractAddress: string, itemId: string, opts?: Opts): Promise<ItemMeta | null> {
  const json = await c(opts).market.items<
    DataTotal<{
      name?: string;
      thumbnail?: string;
      utility?: string | null;
      urn?: string;
      available?: string | number;
      creator?: string;
      rarity?: string;
      category?: string;
      chainId?: number;
      network?: string;
      data?: { wearable?: { isSmart?: boolean; category?: string; bodyShapes?: string[] }; emote?: { category?: string; loop?: boolean; hasSound?: boolean; hasGeometry?: boolean } };
    }>
  >({ contractAddress, itemId, first: 1 }, o(opts));
  const row = json.data?.[0];
  if (!row) return null;
  return {
    name: row.name,
    thumbnail: row.thumbnail,
    isSmart: !!row.data?.wearable?.isSmart,
    utility: row.utility?.trim() || null,
    urn: row.urn ?? null,
    wearableCategory: row.data?.wearable?.category ?? row.data?.emote?.category ?? null,
    gender: toGender(row.data?.wearable?.bodyShapes),
    available: toSupply(row.available),
    creator: row.creator ?? null,
    rarity: row.rarity ?? null,
    category: row.category ?? null,
    chainId: row.chainId ?? null,
    network: row.network ?? null,
    ...(row.data?.emote
      ? { emoteLoop: row.data.emote.loop, emoteHasSound: row.data.emote.hasSound, emoteHasProps: row.data.emote.hasGeometry }
      : {}),
  };
}
