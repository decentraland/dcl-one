// Which Shop surfaces are on: creator sales follow decentraland.org's rollout flag, secondary sales stay on here.
import { afterEach, describe, expect, it } from "vitest";

import { shopFlags } from "./flags";
import { primeRemoteFlags } from "./lib/remoteFlags";

type FlagWindow = Window & { __SHOP_FLAGS__?: { creatorSales?: boolean; secondarySales?: boolean } };

afterEach(() => {
  primeRemoteFlags(null);
  delete (window as FlagWindow).__SHOP_FLAGS__;
});

describe("the Shop's switches", () => {
  it("shows creator sales only once decentraland.org's dapps-shop-creator-sales answers yes", () => {
    expect(shopFlags()).toEqual({ creatorSales: false, secondarySales: true });
    primeRemoteFlags({ "dapps-shop-creator-sales": true });
    expect(shopFlags().creatorSales).toBe(true);
    primeRemoteFlags({ "dapps-shop-creator-sales": false });
    expect(shopFlags().creatorSales).toBe(false);
  });

  it("keeps secondary sales on whatever upstream's flag says, and lets a host override either", () => {
    primeRemoteFlags({ "dapps-shop-secondary-sales": false, "dapps-shop-creator-sales": true });
    expect(shopFlags().secondarySales).toBe(true);
    (window as FlagWindow).__SHOP_FLAGS__ = { creatorSales: false, secondarySales: false };
    expect(shopFlags()).toEqual({ creatorSales: false, secondarySales: false });
  });
});
