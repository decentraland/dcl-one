// from: shop@94584a5c:app/src/hooks/useCreatorSalesEnabled.ts, app/src/hooks/useSecondarySales.ts, app/src/lib/iap.ts

export type ShopFlags = {
  creatorSales: boolean;
  secondarySales: boolean;
};

const DEFAULTS: ShopFlags = { creatorSales: false, secondarySales: true };

type FlagWindow = Window & typeof globalThis & { __SHOP_FLAGS__?: Partial<ShopFlags> };

export function shopFlags(): ShopFlags {
  const fromWindow = typeof window !== "undefined" ? (window as FlagWindow).__SHOP_FLAGS__ : undefined;
  return { ...DEFAULTS, ...(fromWindow ?? {}) };
}

export function isIapMode(): boolean {
  try {
    return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("view") === "mobile-iap";
  } catch {
    return false;
  }
}
