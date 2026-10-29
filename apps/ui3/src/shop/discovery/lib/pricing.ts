// from: shop@94584a5c:app/src/lib/pricing.ts
// Whether a listing row is for sale at all, independent of how it is paid: a CollectionStore mint has
// no trade and is for sale while it has stock.

export function isListingForSale(row: {
  tradeId?: string | null;
  acquisition?: "trade" | "store" | null;
  available?: number | null;
}): boolean {
  if (row.acquisition === "store") return (row.available ?? 0) > 0;
  return !!row.tradeId;
}
