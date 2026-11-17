// from: shop@94584a5c:app/src/hooks/useCreatorSalesEnabled.ts, app/src/lib/iap.ts
// The Shop's rollout switches, from decentraland.org's `dapps` flags. Creator sales (`dapps-shop-creator-sales`, as
// upstream's) is off until the file answers yes. A host (the native shop, a journey) may declare it through
// `window.__SHOP_FLAGS__`. The Marketplace's switch (`dapps-shop-marketplace-features`, dcl-one's own) may be declared
// there too, but only market/access.tsx reads it: a declared value stands at once (else it waits for the file), and
// the Marketplace still needs a web3 wallet session. So may My Store's (`dapps-shop-my-store`, upstream's), read only by
// seller/lib/featureFlags.ts: a declared value stands at once, and the file's address list then does not apply.
//
// The browser host's switches: what the page declares (`window.__SHOP_FLAGS__`) over the file as this page holds it
// (lib/remoteFlags.ts). How the two combine is lib/rolloutFlags.ts.

import { remoteFlag } from "./lib/remoteFlags";
import { declaredOr, resolveShopFlags, type HostShopFlags, type ShopFlags } from "@shop/discovery/lib/rolloutFlags";

export type { HostShopFlags, ShopFlags };

declare global {
  interface Window {
    __SHOP_FLAGS__?: HostShopFlags;
  }
}

/** A host's declared switches, if it declared any. */
export function hostShopFlags(): HostShopFlags {
  return (typeof window !== "undefined" ? window.__SHOP_FLAGS__ : undefined) ?? {};
}

/** A switch as a host's declaration says, else as the file answered (`null` while it has not). */
export function hostOr(key: keyof HostShopFlags, remote: boolean | null): boolean | null {
  return declaredOr(hostShopFlags(), key, remote);
}

export function shopFlags(): ShopFlags {
  return resolveShopFlags(hostShopFlags(), remoteFlag);
}

export function isIapMode(): boolean {
  try {
    return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("view") === "mobile-iap";
  } catch {
    return false;
  }
}
