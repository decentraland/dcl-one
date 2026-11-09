// from: shop@94584a5c:app/src/lib/featureFlags.ts (getIsCreatorSalesEnabled, getIsMyStoreEnabled),
//       app/src/hooks/{useSecondarySales,useCreatorSalesEnabled,useMyStoreEnabled}.ts
// Upstream's feature flags stay capabilities (SHOP.md): the UI hides what a node does not offer, and the
// server checks the same capability on every entry. The node's capability report is not served yet, so a
// host declares them (`window.__SHOP_CAPABILITIES__` or `setShopCapabilities`) and the defaults below say
// what the unified Shop offers. Two changes from upstream's defaults: resales are part of the unified Shop
// (MANA only), so they are on; and a capability that is off or unknown hides its control, never enables
// another payment method.

import { useSyncExternalStore } from "react";

import { hostOr, hostShopFlags } from "../../discovery/flags";
import { REMOTE_FLAG, remoteFlagVariant, useRemoteFlagState } from "../../discovery/lib/remoteFlags";

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

/** What this node offers: on for everyone unless the capability lists addresses. */
export function myStoreAccess(caps: ShopCapabilities, address: string | null): "on" | "off" {
  if (!caps.myStore) return "off";
  if (caps.myStoreOnly.length === 0) return "on";
  return address && caps.myStoreOnly.some((a) => a.toLowerCase() === address.toLowerCase()) ? "on" : "off";
}

export type MyStoreAccess = "pending" | "on" | "off";

/**
 * Upstream's `parseAddressList` (shop@3a322cf9): entries split on commas or whitespace (a dashboard field pasted into
 * by hand), lowercased, only well-formed addresses, each once.
 */
export function parseAddressList(value: string): string[] {
  return [...new Set(value.split(/[\s,]+/).map((a) => a.toLowerCase().trim()).filter((a) => /^0x[0-9a-f]{40}$/.test(a)))];
}

/**
 * Upstream's `useMyStoreAccess` (shop@3a322cf9) over this node's capability: My Store opens only while decentraland.org's
 * `shop-my-store` flag is on; a variant naming accounts narrows it to them, decided once the wallet has resolved.
 * "pending" while the flag, or the wallet such a list needs, has not answered yet.
 */
export function myStoreAccessWith(
  flag: boolean | null,
  variant: string | null,
  caps: ShopCapabilities,
  address: string | null,
  resolving: boolean,
): MyStoreAccess {
  if (flag === null) return "pending";
  if (!flag) return "off";
  const only = variant ? parseAddressList(variant) : [];
  if (only.length > 0) {
    if (resolving) return "pending";
    if (!address || !only.includes(address.toLowerCase())) return "off";
  }
  return myStoreAccess(caps, address);
}

/**
 * {@link myStoreAccessWith} for the signed-in account, kept current as the flags answer. A host's declared `myStore`
 * (`window.__SHOP_FLAGS__`, discovery/flags.ts) stands for the flag, without the file's address list.
 */
export function useMyStoreAccess(address: string | null, resolving: boolean): MyStoreAccess {
  const caps = useShopCapabilities();
  const remote = useRemoteFlagState(REMOTE_FLAG.myStore);
  const declared = typeof hostShopFlags().myStore === "boolean";
  return myStoreAccessWith(hostOr("myStore", remote), declared ? null : remoteFlagVariant(REMOTE_FLAG.myStore), caps, address, resolving);
}
