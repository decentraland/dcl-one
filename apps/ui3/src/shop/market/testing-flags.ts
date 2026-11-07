// Tests of the Marketplace's surfaces turn its rollout on the way a host does (window.__SHOP_FLAGS__); a visitor still
// needs a web3 session to see them (market/access.tsx). Returns the teardown, so `beforeEach(() => marketplaceOn())`
// restores. Never `beforeEach(marketplaceOn)`: Vitest would pass its test context as `on`.
import "../discovery/flags";

export function marketplaceOn(on = true): () => void {
  const before = window.__SHOP_FLAGS__;
  window.__SHOP_FLAGS__ = { ...before, marketplaceFeatures: on };
  return () => {
    window.__SHOP_FLAGS__ = before;
  };
}
