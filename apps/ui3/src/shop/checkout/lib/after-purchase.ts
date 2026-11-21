// from: shop@94584a5c:app/src/lib/after-purchase.ts
// Everything that goes stale the moment a purchase settles, in one list: the balance, the item's
// listing and availability, the buyer's holdings and history. `item` scopes the per-item keys when
// exactly one thing was bought; a basket invalidates them broadly.

import type { QueryClient } from "@tanstack/react-query";
import { CREDITS_BALANCE_QUERY_KEY } from "@shop/credits/balance-query";

export const SHOP_QUERY_KEYS = {
  balance: CREDITS_BALANCE_QUERY_KEY,
  manaBalance: "shop-mana-balance",
  purchases: "shop-purchases",
  creditOrders: "shop-credit-orders",
  activity: "shop-activity",
} as const;

export function invalidateAfterPurchase(qc: QueryClient, item?: { contractAddress: string; tokenId?: string; itemId?: string | null }): void {
  const scoped = (key: string, ...parts: (string | null | undefined)[]) => void qc.invalidateQueries({ queryKey: item ? [key, ...parts] : [key] });
  // The pages key a token by its contract in lowercase.
  const contract = item?.contractAddress.toLowerCase();
  void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.balance] });
  void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.manaBalance] });
  void qc.invalidateQueries({ queryKey: ["detail-trade"] });
  void qc.invalidateQueries({ queryKey: ["shop-item"] });
  scoped("owned-token", contract, item?.tokenId);
  scoped("public-token", contract, item?.tokenId);
  scoped("item-resales", contract, item?.itemId);
  // A LAND's page reads the token and its open order under keys of its own.
  scoped("land-token", contract, item?.tokenId);
  scoped("land-token-order", contract, item?.tokenId);
  void qc.invalidateQueries({ queryKey: ["shop-items"] });
  void qc.invalidateQueries({ queryKey: ["catalog-items"] });
  void qc.invalidateQueries({ queryKey: ["overview-listings"] });
  void qc.invalidateQueries({ queryKey: ["upsell-listings"] });
  void qc.invalidateQueries({ queryKey: ["my-assets"] });
  void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.purchases] });
  void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.activity] });
  void qc.invalidateQueries({ queryKey: ["owned-item-count"] });
}
