import { afterEach, describe, expect, it } from "vitest";

import { shopFlags } from "@ui/shop/discovery/flags";
import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";

import { primeShopPageData } from "./page";

afterEach(() => primeRemoteFlags(null));

describe("a Shop page's server data", () => {
  it("primes the flags the server rendered with, so hydration takes the same branches", () => {
    primeShopPageData({ page: "overview", seo: { title: "Shop", description: "" } as never, queries: [], featureFlags: { "dapps-shop-creator-sales": true } });
    expect(shopFlags().creatorSales).toBe(true);
  });

  it("leaves the flags alone when the data carries none", () => {
    primeRemoteFlags({ "dapps-shop-creator-sales": true });
    primeShopPageData({ page: "overview", seo: { title: "Shop", description: "" } as never, queries: [] });
    expect(shopFlags().creatorSales).toBe(true);
  });
});
