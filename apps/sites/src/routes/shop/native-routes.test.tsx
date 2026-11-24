import { createMemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LAND_NETWORKS } from "@shop/land/contracts";
import { CHAIN } from "@shop/model/chain";

import type { ShopHost } from "@ui/shop/native-contract";

import { nativeShopRoutes } from "./native-routes";

const routeOptions: unknown[] = [];
vi.mock("@ui/shop/routes", async (original) => {
  const actual = await original<typeof import("@ui/shop/routes")>();
  return {
    ...actual,
    shopRouteObjects(...args: Parameters<typeof actual.shopRouteObjects>) {
      routeOptions.push(args[0]);
      return actual.shopRouteObjects(...args);
    },
  };
});

const host: ShopHost = { identity: null, path: "/shop", onNavigate() {}, onSignIn() {}, onExternalLink() {} };
const C = "0xabc0000000000000000000000000000000000001";

async function landsOn(path: string): Promise<string> {
  const router = createMemoryRouter(nativeShopRoutes(host), { initialEntries: [path] });
  router.initialize();
  await new Promise<void>((resolve) => {
    const check = () => router.state.initialized && router.state.navigation.state === "idle";
    if (check()) return resolve();
    const off = router.subscribe(() => {
      if (check()) {
        off();
        resolve();
      }
    });
  });
  const { pathname, search } = router.state.location;
  router.dispose();
  return pathname + search;
}

afterEach(() => vi.unstubAllEnvs());

describe("the Explorer's native Shop routes", () => {
  describe("when the deployment settles LAND on Sepolia", () => {
    it("should build the parcel and estate aliases on that network's registries", () => {
      vi.stubEnv("VITE_SHOP_ETHEREUM_CHAIN_ID", String(CHAIN.SEPOLIA));
      const { land, estate } = LAND_NETWORKS[CHAIN.SEPOLIA]!;

      nativeShopRoutes(host);

      expect(routeOptions.at(-1)).toEqual({ contracts: { land, estate } });
    });
  });

  it("serve the Shop pages at their /shop paths", async () => {
    expect(await landsOn(`/shop/item/${C}/3`)).toBe(`/shop/item/${C}/3`);
    expect(await landsOn("/shop/cart")).toBe("/shop/cart");
  });

  it("resolve /shop, v2's tabs and every old /marketplace path to its Shop page", async () => {
    expect(await landsOn("/shop")).toBe("/shop/overview");
    expect(await landsOn("/shop?tab=my-assets")).toBe("/shop/my-items");
    expect(await landsOn("/marketplace")).toBe("/shop/overview");
    expect(await landsOn(`/marketplace/${C}-3`)).toBe(`/shop/item/${C}/3`);
    expect(await landsOn(`/marketplace/contracts/${C}/tokens/9/sell`)).toBe(`/shop/token/${C}/9/sell`);
    expect(await landsOn("/marketplace/credits")).toBe("/shop/credits");
    expect(await landsOn("/elsewhere")).toBe("/shop/overview");
  });
});
