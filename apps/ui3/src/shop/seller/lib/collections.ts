// from: shop@94584a5c:app/src/lib/collections.ts (fetchCollectionSaleState, fetchCreatorSaleState, sanitizeCollectionName),
//       app/src/lib/api.ts (fetchPeggedPrimaryPrices, fetchCreatorPeggedPrimaryPrices)
// Whether each of a creator's items is on sale in the Shop and at what price. These are PRIMARY items (the
// creator's own mints), so a credit price is the Shop's price for them: a USD-pegged listing's exact
// credits, or for a classic MANA listing the server's conversion, kept with its `manaWei` so the page can
// say it is a MANA listing the creator has not moved into the Shop yet.

import type { CatalogItem } from "@catalyst-api/market/CatalogItem";
import type { ShopListing } from "@catalyst-api/market/ShopListing";
import { shopClient, type ShopClient } from "../../api/shop";

export type CollectionSaleState = {
  isOnSale: boolean;
  /** Exact credits for a USD-pegged listing; the server's conversion for a MANA one. */
  priceCredits: number;
  /** MANA wei when the listing is MANA-denominated (a classic listing). */
  manaWei?: string;
  tradeId?: string;
  /** A running creator sale: the list price before the discount, and when it ends (ms). */
  compareAtCredits?: number;
  saleEndsAt?: number;
};

type Peg = { priceCredits: number; tradeId?: string; compareAtCredits?: number; saleEndsAt?: number };

const PAGE = 200;

type CatalogRow = Pick<CatalogItem, "itemId" | "contractAddress" | "isOnSale" | "price" | "tradeId"> & { priceCredits?: number | null };

async function allCatalogRows(scope: { contractAddress: string } | { creator: string }, c: ShopClient): Promise<CatalogRow[]> {
  const all: CatalogRow[] = [];
  for (let skip = 0; ; skip += PAGE) {
    const { data, total } = await c.catalog.items<{ data?: CatalogRow[]; total?: number }>({ ...scope, first: PAGE, skip, includeSocialEmotes: "false" });
    const rows = data ?? [];
    all.push(...rows);
    if (rows.length < PAGE || ((total ?? 0) > 0 && all.length >= (total ?? 0))) return all;
  }
}

async function peggedPrimaryPrices(scope: { contractAddress: string } | { creator: string }, keyOf: (l: ShopListing) => string, c: ShopClient): Promise<Record<string, Peg>> {
  const map: Record<string, Peg> = {};
  for (let skip = 0; ; skip += PAGE) {
    const res = await c.catalog.shop<{ data?: ShopListing[]; total?: number }>({ ...scope, first: PAGE, skip, listingType: "primary" });
    const listings = res.data ?? [];
    for (const l of listings) {
      if (l.listingType !== "primary" || l.itemId == null) continue;
      map[keyOf(l)] = {
        priceCredits: l.priceCredits,
        ...(l.tradeId ? { tradeId: l.tradeId } : {}),
        ...(l.compareAtCredits != null ? { compareAtCredits: l.compareAtCredits } : {}),
        ...(l.saleEndsAt != null ? { saleEndsAt: l.saleEndsAt * 1000 } : {}),
      };
    }
    if (listings.length < PAGE || ((res.total ?? 0) > 0 && skip + listings.length >= (res.total ?? 0))) return map;
  }
}

function toSaleState(row: CatalogRow, peg: Peg | undefined): CollectionSaleState {
  if (peg) return { isOnSale: true, ...peg };
  return { isOnSale: true, priceCredits: row.priceCredits ?? 0, ...(row.price ? { manaWei: row.price } : {}) };
}

/** Upstream `fetchCollectionSaleState`: one collection's sale state, keyed by on-chain item id. */
export async function fetchCollectionSaleState(contractAddress: string, c: ShopClient = shopClient()): Promise<Record<string, CollectionSaleState>> {
  const scope = { contractAddress: contractAddress.toLowerCase() };
  const [pegged, rows] = await Promise.all([peggedPrimaryPrices(scope, (l) => String(l.itemId), c), allCatalogRows(scope, c)]);
  const map: Record<string, CollectionSaleState> = {};
  for (const r of rows) {
    if (r.itemId == null || !r.isOnSale) continue;
    map[String(r.itemId)] = toSaleState(r, pegged[String(r.itemId)]);
  }
  return map;
}

/** Upstream `fetchCreatorSaleState`: every collection of a creator in one read, keyed `<contract>-<itemId>`. */
export async function fetchCreatorSaleState(creator: string, c: ShopClient = shopClient()): Promise<Record<string, CollectionSaleState>> {
  const scope = { creator: creator.toLowerCase() };
  const keyOf = (l: { contractAddress: string; itemId: string | null }) => `${l.contractAddress.toLowerCase()}-${l.itemId}`;
  const [pegged, rows] = await Promise.all([peggedPrimaryPrices(scope, keyOf, c), allCatalogRows(scope, c)]);
  const map: Record<string, CollectionSaleState> = {};
  for (const r of rows) {
    if (r.itemId == null || !r.isOnSale) continue;
    map[keyOf(r)] = toSaleState(r, pegged[keyOf(r)]);
  }
  return map;
}

/** Upstream `sanitizeCollectionName`: `dcl://neon_nights` reads "Neon Nights". */
export function sanitizeCollectionName(name: string): string {
  if (!name.startsWith("dcl://")) return name;
  return name
    .slice("dcl://".length)
    .split("_")
    .map((word) => (word ? word.charAt(0).toUpperCase() + word.slice(1) : ""))
    .join(" ");
}
