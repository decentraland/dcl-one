/** Test and development hosts can override Shop configuration from the page. */
export function pageOverride<T>(name: string): T | undefined {
  if (import.meta.env?.DEV !== true || typeof window === "undefined") return undefined;
  return (window as unknown as Record<string, T | undefined>)[name];
}

export function parseShopChainId(value: string | undefined): number | undefined {
  const id = Number(value);
  return Number.isSafeInteger(id) && id > 0 ? id : undefined;
}

export function shopChainEnvironment() {
  const chains = pageOverride<{ settlementChainId?: number; ethereumChainId?: number }>("__SHOP_CHAIN__");
  const landChain = pageOverride<unknown>("__SHOP_ETHEREUM_CHAIN_ID__");
  return {
    settlementChainId: chains?.settlementChainId ?? parseShopChainId(import.meta.env?.VITE_SHOP_CHAIN_ID),
    ethereumChainId: chains?.ethereumChainId ?? parseShopChainId(import.meta.env?.VITE_SHOP_ETHEREUM_CHAIN_ID),
    landChainId: typeof landChain === "number" ? landChain : undefined,
  };
}
