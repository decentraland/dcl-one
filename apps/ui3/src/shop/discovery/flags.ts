// from: shop@94584a5c:app/src/hooks/useCreatorSalesEnabled.ts, app/src/lib/iap.ts
// The Shop's rollout switches, from decentraland.org's `dapps` flags: creator sales (`dapps-shop-creator-sales`), as
// upstream's, and the Marketplace features (`dapps-shop-marketplace-features`, dcl-one's own). Each is off until the
// file answers yes. A host (the native shop, a journey) may set either through `window.__SHOP_FLAGS__`. Whether a
// visitor sees the Marketplace also needs a web3 wallet session: market/access.tsx decides that.

import { REMOTE_FLAG, remoteFlag } from "./lib/remoteFlags";

export type ShopFlags = {
  creatorSales: boolean;
  marketplaceFeatures: boolean;
};

type FlagWindow = Window & typeof globalThis & { __SHOP_FLAGS__?: Partial<ShopFlags> };

/** A host's declared switches, if it declared any. */
export function hostShopFlags(): Partial<ShopFlags> {
  return (typeof window !== "undefined" ? (window as FlagWindow).__SHOP_FLAGS__ : undefined) ?? {};
}

export function shopFlags(): ShopFlags {
  return { creatorSales: remoteFlag(REMOTE_FLAG.creatorSales), marketplaceFeatures: remoteFlag(REMOTE_FLAG.marketplaceFeatures), ...hostShopFlags() };
}

export function isIapMode(): boolean {
  try {
    return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("view") === "mobile-iap";
  } catch {
    return false;
  }
}
