// Which Shop surfaces are on: creator sales and the Marketplace follow decentraland.org's rollout flags.
import { afterEach, describe, expect, it } from "vitest";

import { shopFlags } from "./flags";
import { primeRemoteFlags } from "./lib/remoteFlags";

type FlagWindow = Window & { __SHOP_FLAGS__?: { creatorSales?: boolean; marketplaceFeatures?: boolean } };

afterEach(() => {
  primeRemoteFlags(null);
  delete (window as FlagWindow).__SHOP_FLAGS__;
});

describe("the Shop's switches", () => {
  it("shows creator sales only once decentraland.org's dapps-shop-creator-sales answers yes", () => {
    expect(shopFlags()).toEqual({ creatorSales: false, marketplaceFeatures: false });
    primeRemoteFlags({ "dapps-shop-creator-sales": true });
    expect(shopFlags().creatorSales).toBe(true);
    primeRemoteFlags({ "dapps-shop-creator-sales": false });
    expect(shopFlags().creatorSales).toBe(false);
  });

  it("turns the Marketplace on only once dapps-shop-marketplace-features answers yes, and lets a host override either", () => {
    primeRemoteFlags({ "dapps-shop-secondary-sales": true, "dapps-shop-creator-sales": true });
    expect(shopFlags().marketplaceFeatures).toBe(false);
    primeRemoteFlags({ "dapps-shop-marketplace-features": true });
    expect(shopFlags().marketplaceFeatures).toBe(true);
    (window as FlagWindow).__SHOP_FLAGS__ = { creatorSales: true, marketplaceFeatures: false };
    expect(shopFlags()).toEqual({ creatorSales: true, marketplaceFeatures: false });
  });
});
