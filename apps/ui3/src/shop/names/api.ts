// from: marketplace@f3956826:webapp/src/modules/vendor/decentraland/lists/api.ts (fetchBannedNames),
// webapp/src/components/NamesPage/NamesPage.tsx (the ENS section of the browse), shop@94584a5c:app/src/lib/names.ts
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// NAME reads: the DAO's banned-names list (explore's `/dcl-lists/banned-names`, not a Shop route),
// NAMEs on resale (`/shop-api/v1/nfts?category=ens`, MANA only) and the account's own NAMEs.

import { catalystBase } from "../../data/catalyst/client";
import type { NftResult } from "@catalyst-api/market/NftResult";
import type { NftsResponseBody } from "@catalyst-api/market/NftsResponseBody";
import type { ProfileName } from "@catalyst-api/market/ProfileName";
import { shopClient, type ShopClient } from "../api/shop";

export type { NftResult };

/** The banned-names list, lowercased. Upstream treats a failed read as "none banned" and lets the server decide. */
export async function fetchBannedNames(opts: { base?: string; fetchImpl?: typeof fetch; signal?: AbortSignal } = {}): Promise<string[]> {
  const doFetch = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  const res = await doFetch(`${catalystBase(opts.base)}/dcl-lists/banned-names`, { method: "POST", headers: { accept: "application/json" }, signal: opts.signal });
  if (!res.ok) throw new Error(`The banned-names list answered ${res.status}`);
  const body = (await res.json()) as { data?: unknown };
  return Array.isArray(body.data) ? body.data.filter((n): n is string => typeof n === "string").map((n) => n.toLowerCase()) : [];
}

export type NameSort = "recently_listed" | "cheapest" | "newest" | "name";

export const NAME_SORT_LABEL: Record<NameSort, string> = {
  recently_listed: "Recently listed",
  cheapest: "Cheapest",
  newest: "Newest",
  name: "Name",
};

export type NameResaleQuery = { search: string; sort: NameSort; onlyOnSale: boolean; page: number };

export const NAMES_PAGE_SIZE = 24;

export function nameResaleQueryFromSearch(search: string): NameResaleQuery {
  const q = new URLSearchParams(search);
  const sort = q.get("sortBy") as NameSort | null;
  const page = Number(q.get("page"));
  return {
    search: q.get("search") ?? "",
    sort: sort && sort in NAME_SORT_LABEL ? sort : "recently_listed",
    onlyOnSale: q.get("onlyOnSale") !== "false",
    page: Number.isSafeInteger(page) && page > 0 ? page : 1,
  };
}

export function nameResaleQueryToSearch(q: NameResaleQuery): string {
  const p = new URLSearchParams();
  if (q.search) p.set("search", q.search);
  if (q.sort !== "recently_listed") p.set("sortBy", q.sort);
  if (!q.onlyOnSale) p.set("onlyOnSale", "false");
  if (q.page > 1) p.set("page", String(q.page));
  const s = p.toString();
  return s ? `?${s}` : "";
}

/** NAMEs as the marketplace's ENS browse lists them: existing registrar tokens, most on resale. */
export async function fetchNameResales(q: NameResaleQuery, opts: { client?: ShopClient; signal?: AbortSignal } = {}): Promise<{ results: NftResult[]; total: number }> {
  const client = opts.client ?? shopClient();
  const query: Record<string, string | number | boolean> = {
    category: "ens",
    first: NAMES_PAGE_SIZE,
    skip: (q.page - 1) * NAMES_PAGE_SIZE,
    sortBy: q.sort,
  };
  if (q.onlyOnSale) query.isOnSale = true;
  if (q.search.trim()) query.search = q.search.trim();
  const body = await client.market.nfts<NftsResponseBody>(query, { signal: opts.signal });
  return { results: body?.data ?? [], total: body?.total ?? 0 };
}

/** The NAMEs an account owns (`/shop-api/v1/users/{address}/names`). */
export async function fetchOwnedNames(address: string, opts: { client?: ShopClient; signal?: AbortSignal } = {}): Promise<ProfileName[]> {
  const client = opts.client ?? shopClient();
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
