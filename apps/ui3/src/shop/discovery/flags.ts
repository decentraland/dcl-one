// from: shop@94584a5c:app/src/hooks/useCreatorSalesEnabled.ts, app/src/hooks/useSecondarySales.ts, app/src/lib/iap.ts
// Creator sales follow decentraland.org's `dapps-shop-creator-sales` rollout, as upstream's does: off until it answers
// yes. Secondary sales stay on here (SHOP.md: one Shop with the full inventory), where upstream's flag is off.
// A host (the native shop) may still set either through `window.__SHOP_FLAGS__`.

import { REMOTE_FLAG, remoteFlag } from "./lib/remoteFlags";

export type ShopFlags = {
  creatorSales: boolean;
  secondarySales: boolean;
};

const DEFAULTS = { secondarySales: true } as const;

type FlagWindow = Window & typeof globalThis & { __SHOP_FLAGS__?: Partial<ShopFlags> };

export function shopFlags(): ShopFlags {
  const fromWindow = typeof window !== "undefined" ? (window as FlagWindow).__SHOP_FLAGS__ : undefined;
  return { ...DEFAULTS, creatorSales: remoteFlag(REMOTE_FLAG.creatorSales), ...(fromWindow ?? {}) };
}

export function isIapMode(): boolean {
  try {
    return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("view") === "mobile-iap";
  } catch {
    return false;
  }
}
