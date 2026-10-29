// from: shop@94584a5c:app/src/lib/featureFlags.ts (getIsSecondarySalesEnabled, getIsCreatorSalesEnabled, getIsMyStoreEnabled),
//       app/src/hooks/{useSecondarySales,useCreatorSalesEnabled,useMyStoreEnabled}.ts
// Upstream's feature flags stay capabilities (SHOP.md): the UI hides what a node does not offer, and the
// server checks the same capability on every entry. The node's capability report is not served yet, so a
// host declares them (`window.__SHOP_CAPABILITIES__` or `setShopCapabilities`) and the defaults below say
// what the unified Shop offers. Two changes from upstream's defaults: resales are part of the unified Shop
// (MANA only), so they are on; and a capability that is off or unknown hides its control, never enables
// another payment method.

import { useSyncExternalStore } from "react";

export type ShopCapabilities = {
  /** Buying and listing resales (MANA only). */
  secondarySales: boolean;
  /** Creators putting their collections on sale (discount coupons). */
  creatorSales: boolean;
  /** The creator's store dashboard; `myStoreOnly` narrows it to listed addresses (empty means everyone). */
  myStore: boolean;
  myStoreOnly: string[];
};

export const DEFAULT_CAPABILITIES: ShopCapabilities = { secondarySales: true, creatorSales: true, myStore: true, myStoreOnly: [] };

type CapWindow = Window & { __SHOP_CAPABILITIES__?: Partial<ShopCapabilities> };

let declared: Partial<ShopCapabilities> | null = null;
const listeners = new Set<() => void>();
let snapshot: ShopCapabilities | null = null;

export function shopCapabilities(): ShopCapabilities {
  if (snapshot) return snapshot;
  const fromWindow = typeof window !== "undefined" ? (window as CapWindow).__SHOP_CAPABILITIES__ : undefined;
  snapshot = { ...DEFAULT_CAPABILITIES, ...(fromWindow ?? {}), ...(declared ?? {}) };
  return snapshot;
}

/** The host's declaration (sites from its loader, the Explorer from the native contract). Null restores the defaults. */
export function setShopCapabilities(next: Partial<ShopCapabilities> | null): void {
  declared = next;
  snapshot = null;
  for (const l of listeners) l();
}

export async function getIsSecondarySalesEnabled(): Promise<boolean> {
  return shopCapabilities().secondarySales;
}

export async function getIsCreatorSalesEnabled(): Promise<boolean> {
  return shopCapabilities().creatorSales;
}

export async function getIsMyStoreEnabled(): Promise<boolean> {
  return shopCapabilities().myStore;
}

export function useShopCapabilities(): ShopCapabilities {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    shopCapabilities,
    shopCapabilities,
  );
}

/** Upstream `useMyStoreAccess`: on for everyone unless the capability lists addresses. */
export function myStoreAccess(caps: ShopCapabilities, address: string | null): "on" | "off" {
  if (!caps.myStore) return "off";
  if (caps.myStoreOnly.length === 0) return "on";
  return address && caps.myStoreOnly.some((a) => a.toLowerCase() === address.toLowerCase()) ? "on" : "off";
}
