// from: shop@94584a5c:app/src/lib/activity.ts
// One chronological feed of what the account did: credit purchases (per-checkout orders), MANA
// purchases (the buyer side of the chain, which the credits feed never sees), secondary sales, and
// credit-pack top-ups. A resale settles in MANA and the seller received MANA, so a sale shows its exact
// MANA amount and never a credit figure (SHOP.md: no credit price on a resale); the unified Shop does not
// route resale proceeds to credits, so no payout row changes that.

import type { CreditOrder, PurchaseRecord } from "../credits/credits";
import { groupPurchases, type PurchaseOrder } from "./purchases";

export type ActivityFilter = "all" | "purchases" | "sales";

/** A completed sale as marketplace-server lists it (`GET /shop-api/v1/sales`). MANA wei, epoch ms. */
export type SaleRecord = {
  id: string;
  buyer: string;
  seller: string;
  contractAddress: string;
  tokenId: string | null;
  itemId: string | null;
  manaWei: string;
  createdAt: number;
  txHash: string;
  category: string;
  chainId: number | null;
};

/** A sale, normalised for the feed: the exact MANA the seller received. */
export type ActivitySale = {
  id: string;
  contractAddress: string;
  tokenId: string | null;
  itemId: string | null;
  counterparty: string;
  manaWei: string;
  createdAt: number;
  txHash: string;
  chainId: number | null;
};

export type ActivityEntry =
  | { kind: "purchase"; id: string; createdAt: number; order: PurchaseOrder }
  | { kind: "sale"; id: string; createdAt: number; sale: ActivitySale }
  | { kind: "credit"; id: string; createdAt: number; order: CreditOrder }
  | { kind: "mana-purchase"; id: string; createdAt: number; sale: SaleRecord };

export function toActivitySale(sale: SaleRecord): ActivitySale {
  return {
    id: sale.id,
    contractAddress: sale.contractAddress,
    tokenId: sale.tokenId,
    itemId: sale.itemId,
    counterparty: sale.buyer,
    manaWei: sale.manaWei,
    createdAt: sale.createdAt,
    txHash: sale.txHash,
    chainId: sale.chainId,
  };
}

/**
 * PENDING and SETTLED intents always show. EXPIRED shows only when a transaction carrying it reached
 * the chain (`submittedTxHash`): a purchase the buyer attempted and got nothing from. An EXPIRED intent
 * nobody submitted is an opened-and-closed buy modal, noise.
 */
function isWorthShowing(purchase: PurchaseRecord): boolean {
  return purchase.status !== "EXPIRED" || !!purchase.submittedTxHash;
}

export function buildActivityFeed(input: {
  purchases: PurchaseRecord[];
  sales: SaleRecord[];
  creditOrders?: CreditOrder[];
  manaPurchases?: SaleRecord[];
}): ActivityEntry[] {
  const purchaseEntries: ActivityEntry[] = groupPurchases(input.purchases.filter(isWorthShowing)).map((order) => ({
    kind: "purchase",
    id: `purchase:${order.id}`,
    createdAt: order.createdAt,
    order,
  }));
  const saleEntries: ActivityEntry[] = input.sales.map((s) => {
    const sale = toActivitySale(s);
    return { kind: "sale", id: `sale:${sale.id}`, createdAt: sale.createdAt, sale };
  });
  const creditEntries: ActivityEntry[] = (input.creditOrders ?? [])
    .filter((o) => o.status !== "failed" && o.status !== "abandoned")
    .map((order) => ({ kind: "credit", id: `credit:${order.id}`, createdAt: order.createdAt, order }));
  const creditsTxs = new Set(input.purchases.flatMap((p) => (p.txHash ? [p.txHash.toLowerCase()] : [])));
  const manaEntries: ActivityEntry[] = (input.manaPurchases ?? [])
    .filter((s) => !creditsTxs.has(s.txHash.toLowerCase()))
    .map((sale) => ({ kind: "mana-purchase", id: `mana-purchase:${sale.id}`, createdAt: sale.createdAt, sale }));
  return [...purchaseEntries, ...saleEntries, ...creditEntries, ...manaEntries].sort(
    (a, b) => b.createdAt - a.createdAt || (a.id < b.id ? -1 : 1),
  );
}

export function filterActivity(entries: ActivityEntry[], filter: ActivityFilter): ActivityEntry[] {
  if (filter === "all") return entries;
  if (filter === "sales") return entries.filter((e) => e.kind === "sale");
  return entries.filter((e) => e.kind === "purchase" || e.kind === "credit" || e.kind === "mana-purchase");
}
