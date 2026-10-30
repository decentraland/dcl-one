// from: shop@94584a5c:app/src/lib/api.ts (fetchCatalogByIds, toCatalogItem: only what the favorites and lists pages render)
// Hydrates marketplace item ids (`<contract>-<itemId>`) into the rows a favorites grid renders, from
// /shop-api/v3/catalog/items in chunks, in the ids' order; ids the catalog no longer knows are absent.
// These are collection ITEMS: when on sale from their creator, the Shop's price is the server's credit
// price (a primary mint). A row on sale with no credit price is an outage of the quote, shown as such,
// never re-priced in MANA here.

import type { CatalogItem } from "@catalyst-api/market/CatalogItem";
import { shopClient, type ShopClient } from "../../api/shop";

export type FavoriteItem = {
  id: string;
  key: string;
  name: string;
  thumbnail: string;
  contractAddress: string;
  itemId: string;
  rarity: string;
  category: string;
  chainId: number;
  creator: string;
  isOnSale: boolean;
  available: number;
  /** The server's credit price for the creator's listing; null when it could not quote one. */
  priceCredits: number | null;
};

type Row = CatalogItem & { priceCredits?: number | null };

export function toFavoriteItem(row: Row): FavoriteItem {
  return {
    id: row.id,
    key: `${row.contractAddress.toLowerCase()}-${row.itemId}`,
    name: row.name,
    thumbnail: row.thumbnail,
    contractAddress: row.contractAddress.toLowerCase(),
    itemId: row.itemId,
    rarity: row.rarity,
    category: row.category,
    chainId: row.chainId,
    creator: row.creator,
    isOnSale: row.isOnSale && row.available > 0,
    available: row.available,
    priceCredits: typeof row.priceCredits === "number" && row.priceCredits > 0 ? row.priceCredits : null,
  };
}

const CHUNK = 50;

export async function fetchCatalogByIds(ids: string[], c: ShopClient = shopClient()): Promise<FavoriteItem[]> {
  if (ids.length === 0) return [];
  const chunks: string[][] = [];
  for (let i = 0; i < ids.length; i += CHUNK) chunks.push(ids.slice(i, i + CHUNK));
  const pages = await Promise.all(chunks.map(async (chunk) => (await c.catalog.items<{ data?: Row[] }>({ id: chunk, first: chunk.length })).data ?? []));
  const byKey = new Map(pages.flat().map((row) => [`${row.contractAddress.toLowerCase()}-${row.itemId}`, toFavoriteItem(row)]));
  return ids.map((id) => byKey.get(id.toLowerCase())).filter((item): item is FavoriteItem => !!item);
}
