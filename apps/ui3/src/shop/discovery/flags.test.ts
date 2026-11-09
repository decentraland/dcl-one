// Which Shop surfaces are on: creator sales and the Marketplace follow decentraland.org's rollout flags.
import { afterEach, describe, expect, it } from "vitest";

import { shopFlags } from "./flags";
import { primeRemoteFlags } from "./lib/remoteFlags";

afterEach(() => {
  primeRemoteFlags(null);
  delete window.__SHOP_FLAGS__;
});

describe("the Shop's switches", () => {
  it("shows creator sales only once decentraland.org's dapps-shop-creator-sales answers yes", () => {
    expect(shopFlags()).toEqual({ creatorSales: false });
    primeRemoteFlags({ "dapps-shop-creator-sales": true });
    expect(shopFlags().creatorSales).toBe(true);
    primeRemoteFlags({ "dapps-shop-creator-sales": false });
    expect(shopFlags().creatorSales).toBe(false);
  });

  it("lets a host declare creator sales either way", () => {
    primeRemoteFlags({ "dapps-shop-creator-sales": true });
    window.__SHOP_FLAGS__ = { creatorSales: false, marketplaceFeatures: true };
    expect(shopFlags()).toEqual({ creatorSales: false });
    primeRemoteFlags({});
    window.__SHOP_FLAGS__ = { creatorSales: true };
    expect(shopFlags()).toEqual({ creatorSales: true });
  });
});
