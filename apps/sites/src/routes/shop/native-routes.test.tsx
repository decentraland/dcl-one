import { createMemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";

import type { ShopHost } from "@ui/shop/native-contract";

import { nativeShopRoutes } from "./native-routes";

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

describe("the Explorer's native Shop routes", () => {
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
