// Tests of the Marketplace's surfaces turn its rollout on the way a host does (window.__SHOP_FLAGS__); a visitor still
// needs a web3 session to see them (market/access.tsx). Returns the teardown, so `beforeEach(marketplaceOn)` restores.
type FlagWindow = Window & { __SHOP_FLAGS__?: Record<string, boolean> };

export function marketplaceOn(on = true): () => void {
  const w = window as FlagWindow;
  const before = w.__SHOP_FLAGS__;
  w.__SHOP_FLAGS__ = { ...before, marketplaceFeatures: on };
  return () => {
    w.__SHOP_FLAGS__ = before;
  };
}
