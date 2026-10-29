// from: shop@94584a5c:app/src/lib/sales.ts
// A seller's own sales history from /shop-api/v1/sales (rows) and /v1/sales/summary (the exact aggregate).
// Counts are one request (`first=1` reads `total`); sums need the rows, so a big store's earnings from rows
// are capped and say so (`truncated`). Every price is MANA wei, whatever the listing was priced in: the
// trade settled in MANA and that is what the seller was paid.

import type { Sale } from "@catalyst-api/market/Sale";
import type { SalesSummary } from "@catalyst-api/market/SalesSummary";
import { shopClient, type ShopClient } from "../../api/shop";

export type { SalesSummary };

export type SaleRow = {
  id: string;
  itemId: string | null;
  contractAddress: string;
  buyer: string;
  seller: string;
  /** MANA wei. */
  price: string;
  /** Milliseconds. */
  timestamp: number;
  type: "mint" | "order" | "bid";
  network: string;
  tokenId: string | null;
};

function toRow(s: Sale): SaleRow {
  const type = s.type === "mint" || s.type === "bid" ? s.type : "order";
  return { id: s.id, itemId: s.itemId, contractAddress: s.contractAddress, buyer: s.buyer, seller: s.seller, price: s.price, timestamp: s.timestamp, type, network: s.network, tokenId: s.tokenId };
}

/** Upstream `weiOf`: a wei figure as a bigint, or zero for a malformed row (never throws in a render). */
export function weiOf(value: string | null | undefined): bigint {
  try {
    return BigInt(value || "0");
  } catch {
    return 0n;
  }
}

/** Upstream `fetchSalesSummary`. */
export async function fetchSalesSummary(filters: { seller: string; from?: number; to?: number }, c: ShopClient = shopClient()): Promise<SalesSummary> {
  const res = await c.market.salesSummary<{ data: SalesSummary }>({ seller: filters.seller.toLowerCase(), from: filters.from, to: filters.to });
  return res.data;
}

export type SalesFilters = {
  seller?: string;
  contractAddress?: string;
  itemId?: string;
  type?: SaleRow["type"];
  from?: number;
  to?: number;
  first?: number;
  skip?: number;
};

async function get(filters: SalesFilters, c: ShopClient): Promise<{ data: SaleRow[]; total: number }> {
  const res = await c.market.sales({
    seller: filters.seller?.toLowerCase(),
    contractAddress: filters.contractAddress,
    itemId: filters.itemId,
    type: filters.type,
    from: filters.from,
    to: filters.to,
    first: filters.first ?? 100,
    skip: filters.skip || undefined,
    sortBy: "recently_sold",
  });
  return { data: (res.data ?? []).map(toRow), total: res.total ?? 0 };
}

/** Upstream `countSales`: how many match, without fetching one. */
export async function countSales(filters: Omit<SalesFilters, "first" | "skip">, c: ShopClient = shopClient()): Promise<number> {
  return (await get({ ...filters, first: 1 }, c)).total;
}

/** Upstream `fetchSalesPage`: one page, for a table that pages. */
export async function fetchSalesPage(
  filters: Omit<SalesFilters, "first" | "skip">,
  page: { first: number; skip: number },
  c: ShopClient = shopClient(),
): Promise<{ rows: SaleRow[]; total: number }> {
  const { data, total } = await get({ ...filters, ...page }, c);
  return { rows: data, total };
}

/** Upstream `fetchSellerSales`: every sale in the window up to `cap`, and whether that cut it short. */
export async function fetchSellerSales(
  filters: Omit<SalesFilters, "first" | "skip">,
  opts: { cap?: number } = {},
  c: ShopClient = shopClient(),
): Promise<{ rows: SaleRow[]; total: number; truncated: boolean }> {
  const PAGE_SIZE = 1000;
  const cap = opts.cap ?? 5000;
  const rows: SaleRow[] = [];
  let total = 0;
  for (let skip = 0; skip < cap; skip += PAGE_SIZE) {
    const first = Math.min(PAGE_SIZE, cap - skip);
    const page = await get({ ...filters, first, skip }, c);
    total = page.total;
    rows.push(...page.data);
    if (page.data.length < first || rows.length >= total) break;
  }
  return { rows, total, truncated: total > rows.length };
}
