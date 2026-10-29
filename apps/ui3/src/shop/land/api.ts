// from: marketplace@f3956826:webapp/src/modules/vendor/decentraland/nft/api.ts (fetch, fetchOne,
// buildNFTQueryString, appendNFTFiltersToQueryParams: the LAND filters), webapp/src/modules/routing/types.ts (SortBy)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// LAND and estate reads through the one Shop client (`GET /shop-api/v1/nfts`). A 503 is the index
// saying it cannot answer yet (the deferred envelope names the missing indexer): the pages show that
// as an outage with its explanation, never as "no LAND".

import type { NftResult } from "@catalyst-api/market/NftResult";
import type { NftsResponseBody } from "@catalyst-api/market/NftsResponseBody";
import { isShopApiError } from "../api/client";
import { shopClient, type ShopClient } from "../api/shop";

export type { NftResult };

export type LandStatusFilter = "all" | "sale" | "rent";
export type LandCategoryFilter = "land" | "parcel" | "estate";

export type LandSort =
  | "newest"
  | "recently_listed"
  | "cheapest"
  | "name"
  | "rental_listing_date"
  | "max_rental_price"
  | "min_rental_price";

export const LAND_SORT_LABEL: Record<LandSort, string> = {
  newest: "Newest",
  recently_listed: "Recently listed",
  cheapest: "Cheapest",
  name: "Name",
  rental_listing_date: "Recently listed for rent",
  max_rental_price: "Most expensive rent",
  min_rental_price: "Cheapest rent",
};

/** The sorts that make sense for a status (upstream offers the rental sorts on "Available to rent"). */
export function landSortsFor(status: LandStatusFilter): LandSort[] {
  if (status === "rent") return ["rental_listing_date", "min_rental_price", "max_rental_price", "name"];
  if (status === "sale") return ["recently_listed", "cheapest", "newest", "name"];
  return ["newest", "name"];
}

export type LandFilters = {
  status: LandStatusFilter;
  category: LandCategoryFilter;
  sort: LandSort;
  search: string;
  /** MANA, as typed. */
  minPrice: string;
  maxPrice: string;
  minEstateSize: string;
  maxEstateSize: string;
  adjacentToRoad: boolean;
  minDistanceToPlaza: string;
  maxDistanceToPlaza: string;
  rentalDays: number[];
  /** Only the signed-in account's LAND. */
  owner: string | null;
  page: number;
};

export const LAND_PAGE_SIZE = 24;

export const DEFAULT_LAND_FILTERS: LandFilters = {
  status: "sale",
  category: "land",
  sort: "recently_listed",
  search: "",
  minPrice: "",
  maxPrice: "",
  minEstateSize: "",
  maxEstateSize: "",
  adjacentToRoad: false,
  minDistanceToPlaza: "",
  maxDistanceToPlaza: "",
  rentalDays: [],
  owner: null,
  page: 1,
};

const num = (v: string | null) => (v && /^\d+(\.\d+)?$/.test(v.trim()) ? v.trim() : "");

/** The filters a /shop/lands URL carries (upstream's browse options, same names). */
export function landFiltersFromSearch(search: string): LandFilters {
  const q = new URLSearchParams(search);
  const onlyOnSale = q.get("onlyOnSale");
  const onlyOnRent = q.get("onlyOnRent");
  const status: LandStatusFilter = onlyOnRent === "true" ? "rent" : onlyOnSale === "false" ? "all" : "sale";
  const section = q.get("section");
  const category: LandCategoryFilter = section === "parcels" || section === "parcel" ? "parcel" : section === "estates" || section === "estate" ? "estate" : "land";
  const sort = q.get("sortBy") as LandSort | null;
  const page = Number(q.get("page"));
  return {
    status,
    category,
    sort: sort && landSortsFor(status).includes(sort) ? sort : landSortsFor(status)[0]!,
    search: q.get("search") ?? "",
    minPrice: num(q.get("minPrice")),
    maxPrice: num(q.get("maxPrice")),
    minEstateSize: num(q.get("minEstateSize")),
    maxEstateSize: num(q.get("maxEstateSize")),
    adjacentToRoad: q.get("adjacentToRoad") === "true",
    minDistanceToPlaza: num(q.get("minDistanceToPlaza")),
    maxDistanceToPlaza: num(q.get("maxDistanceToPlaza")),
    rentalDays: q.getAll("rentalDays").map(Number).filter((d) => Number.isSafeInteger(d) && d > 0),
    owner: null,
    page: Number.isSafeInteger(page) && page > 0 ? page : 1,
  };
}

/** Back to the URL, keeping only what differs from the defaults. */
export function landFiltersToSearch(f: LandFilters): string {
  const q = new URLSearchParams();
  if (f.status === "rent") q.set("onlyOnRent", "true");
  if (f.status === "all") q.set("onlyOnSale", "false");
  if (f.category !== "land") q.set("section", f.category === "parcel" ? "parcels" : "estates");
  if (f.sort !== landSortsFor(f.status)[0]) q.set("sortBy", f.sort);
  if (f.search) q.set("search", f.search);
  for (const key of ["minPrice", "maxPrice", "minEstateSize", "maxEstateSize", "minDistanceToPlaza", "maxDistanceToPlaza"] as const) {
    if (f[key]) q.set(key, f[key]);
  }
  if (f.adjacentToRoad) q.set("adjacentToRoad", "true");
  for (const d of f.rentalDays) q.append("rentalDays", String(d));
  if (f.page > 1) q.set("page", String(f.page));
  const s = q.toString();
  return s ? `?${s}` : "";
}

const WEI = 10n ** 18n;

function manaToWeiString(mana: string): string {
  const [whole = "0", frac = ""] = mana.split(".");
  return (BigInt(whole) * WEI + BigInt((frac + "0".repeat(18)).slice(0, 18) || "0")).toString();
}

/** The `/v1/nfts` query for the filters (upstream `buildNFTQueryString` for the LAND section). */
export function landNftsQuery(f: LandFilters): Record<string, string | number | boolean | string[]> {
  const query: Record<string, string | number | boolean | string[]> = {
    first: LAND_PAGE_SIZE,
    skip: (f.page - 1) * LAND_PAGE_SIZE,
    sortBy: f.sort,
  };
  if (f.category === "land") query.isLand = true;
  else query.category = f.category;
  if (f.owner) query.owner = f.owner.toLowerCase();
  if (f.status === "sale") query.isOnSale = true;
  if (f.status === "rent") query.isOnRent = true;
  if (f.search.trim()) query.search = f.search.trim();
  if (f.minPrice) query.minPrice = manaToWeiString(f.minPrice);
  if (f.maxPrice) query.maxPrice = manaToWeiString(f.maxPrice);
  if (f.minEstateSize) query.minEstateSize = f.minEstateSize;
  if (f.maxEstateSize) query.maxEstateSize = f.maxEstateSize;
  if (f.minDistanceToPlaza) query.minDistanceToPlaza = f.minDistanceToPlaza;
  if (f.maxDistanceToPlaza) query.maxDistanceToPlaza = f.maxDistanceToPlaza;
  if (f.adjacentToRoad) query.adjacentToRoad = true;
  if (f.status === "rent" && f.rentalDays.length) query.rentalDays = f.rentalDays.map(String);
  return query;
}

export type IndexOutage = { message: string; source: string | null };

/** A 503 from the index: the explanation to show instead of an empty list. */
export function indexOutage(e: unknown): IndexOutage | null {
  if (!isShopApiError(e) || e.status !== 503) return null;
  const body = e.body as { message?: unknown; source?: unknown } | null;
  return {
    message: typeof body?.message === "string" && body.message ? body.message : "The LAND index is catching up.",
    source: typeof body?.source === "string" ? body.source : null,
  };
}

export async function fetchLandNfts(
  f: LandFilters,
  opts: { client?: ShopClient; signal?: AbortSignal } = {},
): Promise<{ results: NftResult[]; total: number }> {
  const client = opts.client ?? shopClient();
  const body = await client.market.nfts<NftsResponseBody>(landNftsQuery(f), { signal: opts.signal });
  return { results: body?.data ?? [], total: body?.total ?? 0 };
}

/** One LAND, estate or NAME token with its open order and rental (upstream `fetchOne`). */
export async function fetchNftResult(
  contractAddress: string,
  tokenId: string,
  opts: { client?: ShopClient; signal?: AbortSignal } = {},
): Promise<NftResult | null> {
  const client = opts.client ?? shopClient();
  const body = await client.market.nfts<NftsResponseBody>({ contractAddress: contractAddress.toLowerCase(), tokenId, first: 1 }, { signal: opts.signal });
  return body?.data?.[0] ?? null;
}

/** The account's own LAND, parcels and estates, for the map's "Owned by me" layer. */
export async function fetchOwnedLand(owner: string, opts: { client?: ShopClient; signal?: AbortSignal } = {}): Promise<NftResult[]> {
  const client = opts.client ?? shopClient();
  const body = await client.market.nfts<NftsResponseBody>({ owner: owner.toLowerCase(), isLand: true, first: 1000, skip: 0 }, { signal: opts.signal });
  return body?.data ?? [];
}

export type LandNftData =
  | { kind: "parcel"; x: number; y: number; description: string | null; estate: { name: string; tokenId: string } | null }
  | { kind: "estate"; size: number; description: string | null; parcels: { x: number; y: number }[] }
  | { kind: "name"; subdomain: string }
  | { kind: "other" };

/** The typed payload of a LAND, estate or NAME NFT. */
export function landData(result: NftResult): LandNftData {
  const data = result.nft.data as Record<string, unknown>;
  if ("parcel" in data) {
    const p = data.parcel as { x: string; y: string; description: string | null; estate: { name: string; tokenId: string } | null };
    return { kind: "parcel", x: Number(p.x), y: Number(p.y), description: p.description, estate: p.estate };
  }
  if ("estate" in data) {
    const e = data.estate as { size: number; description: string | null; parcels: { x: number; y: number }[] };
    return { kind: "estate", size: e.size, description: e.description, parcels: e.parcels };
  }
  if ("ens" in data) return { kind: "name", subdomain: (data.ens as { subdomain: string }).subdomain };
  return { kind: "other" };
}

/** The parcels a LAND NFT covers. */
export function landParcels(result: NftResult): { x: number; y: number }[] {
  const d = landData(result);
  if (d.kind === "parcel") return [{ x: d.x, y: d.y }];
  if (d.kind === "estate") return d.parcels;
  return [];
}
