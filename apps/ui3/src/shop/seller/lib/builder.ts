// from: shop@94584a5c:app/src/lib/builder.ts
// A creator's published collections and items from the builder server, over the node's /builder-api
// (the Builder boundary: the Shop reads, the Builder authors). Requests are signed with the Shop's
// signer, over the exact path sent, like every other Shop request. The builder UUID is NOT the on-chain
// item id: a listing and a trade name `blockchainItemId`.

import { catalystBase } from "../../../data/catalyst/client";
import type { ShopSigner } from "../../api/signer";

type RawCollection = {
  id: string;
  name: string;
  eth_address: string;
  contract_address: string | null;
  is_published: boolean;
  is_approved: boolean;
  minters?: string[];
};

type RawItem = {
  id: string;
  collection_id: string;
  contract_address?: string | null;
  blockchain_item_id: string | null;
  name: string;
  thumbnail?: string;
  contents?: Record<string, string>;
  is_published?: boolean;
  is_approved?: boolean;
  total_supply?: string | number;
  created_at?: number | string;
  rarity?: string;
  type?: "wearable" | "emote";
  data?: { category?: string; wearable?: { category?: string }; emote?: { category?: string } };
};

type Paginated<T> = { results: T[] } | { data: T[] } | T[];

function unwrapList<T>(payload: Paginated<T>): T[] {
  if (Array.isArray(payload)) return payload;
  if ("results" in payload && Array.isArray(payload.results)) return payload.results;
  if ("data" in payload && Array.isArray(payload.data)) return payload.data;
  return [];
}

export type CreatorCollection = {
  id: string;
  name: string;
  contractAddress: string;
  isPublished: boolean;
  isApproved: boolean;
  minters: string[];
};

export type PublishableItem = {
  id: string;
  collectionId: string;
  collectionName: string;
  contractAddress: string;
  /** The on-chain item index: what a primary listing's COLLECTION_ITEM names. */
  blockchainItemId: string;
  name: string;
  category: string;
  rarity: string;
  thumbnail: string;
  type: "wearable" | "emote";
  isPublished: boolean;
  isApproved: boolean;
  totalSupply: number;
  maxSupply: number;
  remainingSupply: number;
  createdAt?: number;
  minters: string[];
};

/** @dcl/schemas `Rarity.getMaxSupply`. */
export const RARITY_MAX_SUPPLY: Record<string, number> = {
  common: 100_000,
  uncommon: 10_000,
  rare: 5_000,
  epic: 1_000,
  legendary: 100,
  exotic: 50,
  mythic: 10,
  unique: 1,
};

export type BuilderOptions = { signer: ShopSigner; base?: string; fetchImpl?: typeof fetch };

function builderBase(base?: string): string {
  return `${catalystBase(base)}/builder-api/v1`;
}

async function getJson<T>(path: string, opts: BuilderOptions): Promise<T> {
  const url = `${builderBase(opts.base)}${path}`;
  const signPath = new URL(url, "http://shop.invalid").pathname;
  const headers = { accept: "application/json", ...(await opts.signer.sign("GET", signPath, {})) };
  const res = await (opts.fetchImpl ?? fetch)(url, { method: "GET", headers });
  if (!res.ok) throw new Error(`builder-server ${res.status}: ${await res.text()}`);
  const json = (await res.json()) as { ok?: boolean; data?: T } | T;
  return ((json as { data?: T }).data ?? json) as T;
}

function categoryOf(item: RawItem): string {
  return item.data?.wearable?.category ?? item.data?.emote?.category ?? item.data?.category ?? item.type ?? "wearable";
}

function contentUrl(hash: string, base?: string): string {
  return `${builderBase(base)}/storage/contents/${hash}`;
}

function fromContents(name: string | undefined, contents: Record<string, string> | undefined, base?: string): string {
  if (!name) return "";
  if (/^https?:\/\//.test(name)) return name;
  const hash = contents?.[name];
  if (hash) return contentUrl(hash, base);
  if (/^(ba|Qm)/.test(name)) return contentUrl(name, base);
  return "";
}

/** Upstream `fetchCreatorCollections`: the creator's PUBLISHED collections (the only ones that can be listed). */
export async function fetchCreatorCollections(address: string, opts: BuilderOptions): Promise<CreatorCollection[]> {
  const payload = await getJson<Paginated<RawCollection>>(`/${address.toLowerCase()}/collections?is_published=true`, opts);
  return unwrapList(payload)
    .filter((c) => c.is_published && !!c.contract_address)
    .map((c) => ({
      id: c.id,
      name: c.name,
      contractAddress: (c.contract_address as string).toLowerCase(),
      isPublished: c.is_published,
      isApproved: c.is_approved,
      minters: (c.minters ?? []).map((m) => m.toLowerCase()),
    }));
}

function toPublishableItem(raw: RawItem, collection: CreatorCollection, base?: string): PublishableItem {
  const rarity = raw.rarity ?? "common";
  const total = Number(raw.total_supply ?? 0) || 0;
  const max = RARITY_MAX_SUPPLY[rarity.toLowerCase()] ?? 0;
  const contractAddress = (raw.contract_address ?? collection.contractAddress).toLowerCase();
  return {
    id: raw.id,
    collectionId: collection.id,
    collectionName: collection.name,
    contractAddress,
    blockchainItemId: raw.blockchain_item_id ?? "",
    name: raw.name,
    category: categoryOf(raw),
    rarity,
    thumbnail: fromContents(raw.thumbnail, raw.contents, base) || fromContents("thumbnail.png", raw.contents, base),
    type: raw.type ?? "wearable",
    isPublished: raw.is_published ?? collection.isPublished,
    isApproved: raw.is_approved ?? collection.isApproved,
    totalSupply: total,
    maxSupply: max,
    remainingSupply: Math.max(0, max - total),
    createdAt: raw.created_at ? Number(raw.created_at) * 1000 : undefined,
    minters: collection.minters,
  };
}

/** Upstream `isPublished`: published, approved and minted into an on-chain item index. */
export function isPublished(item: PublishableItem): boolean {
  return item.isPublished && item.isApproved && item.blockchainItemId !== "" && item.blockchainItemId != null;
}

/** Upstream `isPublishable`: published with copies left to sell. */
export function isPublishable(item: PublishableItem): boolean {
  return isPublished(item) && item.remainingSupply > 0;
}

/** Upstream `fetchCollectionItems`: the items of one collection. */
export async function fetchCollectionItems(
  collection: CreatorCollection,
  opts: BuilderOptions & { includeSoldOut?: boolean },
): Promise<PublishableItem[]> {
  const payload = await getJson<Paginated<RawItem>>(`/collections/${collection.id}/items`, opts);
  return unwrapList(payload)
    .map((raw) => toPublishableItem(raw, collection, opts.base))
    .filter(opts.includeSoldOut ? isPublished : isPublishable);
}

/**
 * Upstream `fetchPublishableItems`: every listable creation, from one address-wide read with its
 * collections; falls back to one read per collection when the address-wide read fails.
 */
export async function fetchPublishableItems(address: string, opts: BuilderOptions & { includeSoldOut?: boolean }): Promise<PublishableItem[]> {
  const [collections, rawItems] = await Promise.all([
    fetchCreatorCollections(address, opts),
    getJson<Paginated<RawItem>>(`/${address.toLowerCase()}/items`, opts).then(unwrapList, () => null),
  ]);
  if (collections.length === 0) return [];
  if (rawItems === null) {
    const perCollection = await Promise.all(collections.map((c) => fetchCollectionItems(c, opts).catch(() => [] as PublishableItem[])));
    return perCollection.flat();
  }
  const byId = new Map(collections.map((c) => [c.id, c]));
  const position = new Map(collections.map((c, i) => [c.id, i]));
  return rawItems
    .flatMap((raw) => {
      const collection = raw.collection_id ? byId.get(raw.collection_id) : undefined;
      return collection ? [toPublishableItem(raw, collection, opts.base)] : [];
    })
    .filter(opts.includeSoldOut ? isPublished : isPublishable)
    .sort((a, b) => (position.get(a.collectionId) ?? 0) - (position.get(b.collectionId) ?? 0));
}
