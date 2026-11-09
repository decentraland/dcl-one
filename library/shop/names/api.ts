// from: marketplace@f3956826:webapp/src/modules/vendor/decentraland/lists/api.ts (fetchBannedNames),
// webapp/src/components/NamesPage/NamesPage.tsx (the ENS section of the browse), shop@94584a5c:app/src/lib/names.ts
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// NAME reads: the DAO's banned-names list (explore's `/dcl-lists/banned-names`, not a Shop route),
// NAMEs on resale (`/shop-api/v1/nfts?category=ens`, MANA only), the account's own NAMEs and a NAME's credits
// quote. The host gives the node's base, the transport and the Shop client (names/reads.ts in the browser).

import type { NftResult } from "@catalyst-api/market/NftResult";
import type { NftsResponseBody } from "@catalyst-api/market/NftsResponseBody";
import type { ProfileName } from "@catalyst-api/market/ProfileName";
import { isShopApiError } from "@shop/api/client";
import { type ShopClient } from "@shop/api/shop";
import { centsFromCredits, parseManaInput, type UsdCents } from "@shop/model/money";
import type { MissingFact } from "@shop/model/settlement";
import { NAME_PRICE_IN_WEI } from "./name";

export type { NftResult };

/** How a host reads a list from the node: the request as given, its status and body as text. */
export type ListsTransport = (
  url: string,
  init: { method: "POST"; headers: Record<string, string>; signal?: AbortSignal },
) => Promise<{ status: number; body: string }>;

/**
 * The banned-names list at `base` (the node, normalized by the host), lowercased. A non-2xx answer is an error. Upstream
 * treats a failed read as "none banned" and lets the server decide.
 */
export async function fetchBannedNames(opts: { base: string; transport: ListsTransport; signal?: AbortSignal }): Promise<string[]> {
  const res = await opts.transport(`${opts.base}/dcl-lists/banned-names`, { method: "POST", headers: { accept: "application/json" }, signal: opts.signal });
  if (res.status < 200 || res.status >= 300) throw new Error(`The banned-names list answered ${res.status}`);
  const body = JSON.parse(res.body) as { data?: unknown };
  return Array.isArray(body.data) ? body.data.filter((n): n is string => typeof n === "string").map((n) => n.toLowerCase()) : [];
}

export type NameSort = "recently_listed" | "cheapest" | "newest" | "name";

export const NAME_SORT_LABEL: Record<NameSort, string> = {
  recently_listed: "Recently listed",
  cheapest: "Cheapest",
  newest: "Newest",
  name: "Name",
};

/** `minPrice`/`maxPrice`: MANA as typed ("12.5"), as the marketplace keeps them in its URL; "" is no bound. */
export type NameResaleQuery = { search: string; sort: NameSort; onlyOnSale: boolean; minPrice: string; maxPrice: string; page: number };

const MAX_WEI = 2n ** 256n - 1n;

/**
 * A typed price bound: blank or zero is no bound (`wei: null`), a positive MANA amount whose wei fits a
 * uint256 is that wei, and anything else is not a bound at all (null).
 */
export function parsePriceBound(text: string): { wei: bigint | null } | null {
  const t = text.trim();
  if (t === "" || /^0+(\.0*)?$/.test(t)) return { wei: null };
  const wei = parseManaInput(t);
  return wei !== null && wei <= MAX_WEI ? { wei } : null;
}

/** A price bound as the URL may carry it: kept when it bounds anything, else dropped. */
function priceBound(raw: string | null): string {
  const t = (raw ?? "").trim();
  return parsePriceBound(t)?.wei ? t : "";
}

export const NAMES_PAGE_SIZE = 24;

export function nameResaleQueryFromSearch(search: string): NameResaleQuery {
  const q = new URLSearchParams(search);
  const sort = q.get("sortBy") as NameSort | null;
  const page = Number(q.get("page"));
  return {
    search: q.get("search") ?? "",
    sort: sort && sort in NAME_SORT_LABEL ? sort : "recently_listed",
    onlyOnSale: q.get("onlyOnSale") !== "false",
    minPrice: priceBound(q.get("minPrice")),
    maxPrice: priceBound(q.get("maxPrice")),
    page: Number.isSafeInteger(page) && page > 0 ? page : 1,
  };
}

export function nameResaleQueryToSearch(q: NameResaleQuery): string {
  const p = new URLSearchParams();
  if (q.search) p.set("search", q.search);
  if (q.sort !== "recently_listed") p.set("sortBy", q.sort);
  if (!q.onlyOnSale) p.set("onlyOnSale", "false");
  if (q.minPrice) p.set("minPrice", q.minPrice);
  if (q.maxPrice) p.set("maxPrice", q.maxPrice);
  if (q.page > 1) p.set("page", String(q.page));
  const s = p.toString();
  return s ? `?${s}` : "";
}

/** NAMEs as the marketplace's ENS browse lists them: existing registrar tokens, most on resale. */
export async function fetchNameResales(q: NameResaleQuery, opts: { client: ShopClient; signal?: AbortSignal }): Promise<{ results: NftResult[]; total: number }> {
  const { client } = opts;
  const query: Record<string, string | number | boolean> = {
    category: "ens",
    first: NAMES_PAGE_SIZE,
    skip: (q.page - 1) * NAMES_PAGE_SIZE,
    sortBy: q.sort,
  };
  if (q.onlyOnSale) query.isOnSale = true;
  if (q.search.trim()) query.search = q.search.trim();
  // The route's price bounds are MANA in wei (marketplace-server's unit) on the open listing. A range asks
  // for live listings only: the subgraph keeps an expired order's price, the index does not.
  const min = parsePriceBound(q.minPrice)?.wei;
  const max = parsePriceBound(q.maxPrice)?.wei;
  if (min) query.minPrice = min.toString();
  if (max) query.maxPrice = max.toString();
  if (min || max) query.isOnSale = true;
  const body = await client.market.nfts<NftsResponseBody>(query, { signal: opts.signal });
  return { results: body?.data ?? [], total: body?.total ?? 0 };
}

/** The NAMEs an account owns (`/shop-api/v1/users/{address}/names`). */
export async function fetchOwnedNames(address: string, opts: { client: ShopClient; signal?: AbortSignal }): Promise<ProfileName[]> {
  const { client } = opts;
  const body = await client.market.userAssets<{ data?: { elements?: ProfileName[] } } | { elements?: ProfileName[] }>(
    address,
    "names",
    { pageNum: 1, pageSize: 100 },
    { signal: opts.signal },
  );
  const envelope = body as { data?: { elements?: ProfileName[] }; elements?: ProfileName[] };
  return envelope.data?.elements ?? envelope.elements ?? [];
}

/** `name.dcl.eth` of a NAME NFT (its ENS subdomain), or its display name. */
export function nameOf(result: NftResult): string {
  const data = result.nft.data as { ens?: { subdomain?: string } };
  return data.ens?.subdomain ?? result.nft.name;
}

/** The credits price of 100 MANA in cents (whole credits), or the reason it cannot be given. */
export async function quoteNameCredits(client: ShopClient, signal?: AbortSignal): Promise<{ quoted: UsdCents } | { missing: MissingFact }> {
  try {
    const body = await client.credits.quotes({ amounts: [NAME_PRICE_IN_WEI] }, { signal });
    const raw = body?.amounts?.[0];
    const credits = raw != null && /^[0-9]+$/.test(raw) ? Number(raw) : null;
    const cents = credits !== null && credits > 0 ? centsFromCredits(credits) : null;
    return cents !== null ? { quoted: cents } : { missing: "manaUsdRate" };
  } catch (e) {
    if ((e as { name?: string } | null)?.name === "AbortError") throw e;
    if (isShopApiError(e) || e instanceof Error) return { missing: "manaUsdRate" };
    throw e;
  }
}
