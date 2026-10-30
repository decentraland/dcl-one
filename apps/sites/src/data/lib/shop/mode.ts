// Which Shop API paths a node answers: the canonical /shop-api, or (SHOP_API_MODE=legacy, or the build's
// VITE_SHOP_API_MODE) the legacy aliases (/marketplace-api, /credits, ...) while it has not mounted
// /shop-api. The SSR loaders read it here; root hands it to the browser as window.__SHOP_API_MODE__,
// which ui3's one Shop client reads (ui3/src/shop/api/client.ts).

import type { ShopApiMode } from "@ui/shop/api/client";

import { processEnv, type BaseEnv } from "../catalyst/bases";

export function serverShopApiMode(env: BaseEnv = processEnv()): ShopApiMode {
  const raw = (env.SHOP_API_MODE ?? env.VITE_SHOP_API_MODE ?? "").trim();
  return raw === "legacy" ? "legacy" : "canonical";
}
