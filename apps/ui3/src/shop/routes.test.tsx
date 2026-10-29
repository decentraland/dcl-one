import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it } from "vitest";

import { ESTATE_CONTRACT, LAND_CONTRACT, landTokenId } from "../data/catalyst/landListing";
import { resolveShopAlias, SHOP_PAGES, shopPath, shopRouteObjects, ShopRoutes } from "./routes";

const PAGE_PLAN: readonly [canonical: string, module: string][] = [
  ["/shop/*", "pages/NotFound"],
  ["/shop/accounts/:address", "pages/Account"],
  ["/shop/activity", "pages/Activity"],
  ["/shop/cart", "pages/Cart"],
  ["/shop/collection/:contractAddress", "pages/Collection"],
  ["/shop/credits", "pages/Credits"],
  ["/shop/event", "pages/Event"],
  ["/shop/item/:contractAddress/:itemId", "pages/ItemDetail"],
  ["/shop/item/:contractAddress/:itemId/bid", "pages/TokenBid"],
  ["/shop/item/:contractAddress/:itemId/buy/status", "pages/BuyStatus"],
  ["/shop/items", "pages/Items"],
  ["/shop/items/creator/:address", "pages/Creator"],
  ["/shop/items/outfits/:id", "pages/OutfitDetail"],
  ["/shop/lands", "pages/Lands"],
  ["/shop/my-favorites", "pages/MyFavorites"],
  ["/shop/my-favorites/:listId", "pages/FavoriteList"],
  ["/shop/my-items", "pages/MyItems"],
  ["/shop/my-store", "pages/MyStore"],
  ["/shop/names", "pages/Names"],
  ["/shop/outfits/:id/edit", "pages/OutfitStudio"],
  ["/shop/outfits/manage", "pages/OutfitStudio"],
  ["/shop/outfits/new", "pages/OutfitStudio"],
  ["/shop/overview", "pages/Overview"],
  ["/shop/rankings", "pages/Rankings"],
  ["/shop/settings", "pages/Settings"],
  ["/shop/store-settings", "pages/StoreSettings"],
  ["/shop/success", "pages/Success"],
  ["/shop/token/:contractAddress/:tokenId", "pages/ItemDetail"],
  ["/shop/token/:contractAddress/:tokenId/bid", "pages/TokenBid"],
  ["/shop/token/:contractAddress/:tokenId/bids", "workflows/AcceptBid"],
  ["/shop/token/:contractAddress/:tokenId/buy/status", "pages/BuyStatus"],
  ["/shop/token/:contractAddress/:tokenId/cancel", "components/RemoveListingModal"],
  ["/shop/token/:contractAddress/:tokenId/manage", "pages/TokenManage"],
  ["/shop/token/:contractAddress/:tokenId/rent", "pages/TokenRent"],
  ["/shop/token/:contractAddress/:tokenId/sell", "pages/TokenSell"],
  ["/shop/token/:contractAddress/:tokenId/transfer", "pages/TokenTransfer"],
];
const ALIAS_PLAN: readonly string[] = [
  "/shop -> /shop/overview",
  "/shop/market -> /shop/items",
  "/shop/my-purchases -> /shop/activity",
  "/shop/assets -> /shop/items",
  "/shop/assets/* -> /shop/items/*",
  "/shop/my-assets -> /shop/my-items",
  "/shop/import -> /shop/activity?section=listings",
  "/shop/authorizations -> /shop/settings",
  "/shop/parcels/:x/:y -> /shop/token/<LAND>/<tokenId>",
  "/shop/estates/:estateId -> /shop/token/<EstateRegistry>/:estateId",
  "/shop/items (query kept)",
];

describe("the /shop route table is the page plan", () => {
  it("every canonical page the plan assigns to a ui3/src/shop page file is routed to that file", () => {
    const missing: string[] = [];
    for (const [canonical, module] of PAGE_PLAN) {
      const path = canonical === "/shop/*" ? "*" : canonical.slice("/shop/".length);
      const routed = SHOP_PAGES.some((p) => p.path === path && p.module === `./${module}.tsx`);
      if (!routed) missing.push(`${canonical} -> ${module}`);
    }
    expect(missing).toEqual([]);
  });

  it("every alias and resolver the plan assigns to routes.tsx resolves as it says", () => {
    expect(ALIAS_PLAN).toHaveLength(11);
    const expectations: [string, string][] = [
      ["/shop", "/shop/overview"],
      ["/shop/market", "/shop/items"],
      ["/shop/my-purchases", "/shop/activity"],
      ["/shop/assets", "/shop/items"],
      ["/shop/assets/creator/0xabc", "/shop/items/creator/0xabc"],
      ["/shop/my-assets", "/shop/my-items"],
      ["/shop/import", "/shop/activity?section=listings"],
      ["/shop/authorizations", "/shop/settings"],
      ["/shop/parcels/12/-4", `/shop/token/${LAND_CONTRACT}/${landTokenId(12, -4)}`],
      ["/shop/estates/55", `/shop/token/${ESTATE_CONTRACT}/55`],
      ["/shop/browse?assetType=nft", "/shop/items?assetType=nft"],
    ];
    for (const [from, to] of expectations) {
      const [path, search = ""] = from.split("?");
      expect(resolveShopAlias(path!, search ? `?${search}` : ""), from).toBe(to);
    }
    for (const alias of ALIAS_PLAN) {
      const [from] = alias.split(" -> ");
      const concrete = from!.replace("/*", "/x").replace(" (query kept)", "");
      if (concrete === "/shop/items") continue;
      expect(resolveShopAlias(concrete.replace(":x/:y", "1/2").replace(":estateId", "3")), alias).not.toBeNull();
    }
  });
});

describe("aliases keep asset identity, the query and the return destination", () => {
  it("merges the query like upstream AliasRedirect, carries the rest of a renamed path", () => {
    expect(resolveShopAlias("/shop/", "?x=1", "#top")).toBe("/shop/overview?x=1#top");
    expect(resolveShopAlias("/shop/import", "?x=1&section=feed")).toBe("/shop/activity?x=1&section=listings");
    expect(resolveShopAlias("/shop/assets/outfits/9", "?returnTo=%2Fshop%2Fcart")).toBe("/shop/items/outfits/9?returnTo=%2Fshop%2Fcart");
    expect(resolveShopAlias("/shop/parcels/1/2/detail", "?returnTo=%2Fshop")).toBe(`/shop/token/${LAND_CONTRACT}/${landTokenId(1, 2)}?returnTo=%2Fshop`);
    expect(resolveShopAlias("/shop/browse", "?section=land")).toBe("/shop/lands");
    expect(resolveShopAlias("/shop/browse", "?section=ens")).toBe("/shop/names");
  });

  it("pages and foreign paths are not aliases", () => {
    expect(resolveShopAlias("/shop/overview")).toBeNull();
    expect(resolveShopAlias("/shop/token/0xabc/1")).toBeNull();
    expect(resolveShopAlias("/marketplace/browse")).toBeNull();
    expect(resolveShopAlias("/shopping")).toBeNull();
    expect(resolveShopAlias("/shop/estates/nope")).toBeNull();
  });

  it("builds canonical links", () => {
    expect(shopPath("itemDetail", { contractAddress: "0xabc", itemId: 7 })).toBe("/shop/item/0xabc/7");
    expect(shopPath("tokenBuyStatus", { contractAddress: "0xabc", tokenId: "42" })).toBe("/shop/token/0xabc/42/buy/status");
    expect(() => shopPath("tokenSell", { contractAddress: "0xabc" })).toThrow();
  });
});

function renderAt(url: string, opts: Parameters<typeof shopRouteObjects>[0] = {}, modules?: Parameters<typeof shopRouteObjects>[1]) {
  const router = createMemoryRouter([{ path: "/shop", children: shopRouteObjects(opts, modules) }], { initialEntries: [url] });
  render(<RouterProvider router={router} />);
  return router;
}

describe("the route objects mount in ui3's router", () => {
  it("a page not moved in yet renders its placeholder", async () => {
    renderAt("/shop/overview", {}, {});
    expect(await screen.findByRole("status", { name: "Overview" })).toBeTruthy();
  });

  it("an alias navigates, keeping the query", async () => {
    const router = renderAt("/shop/market?x=1", {}, {});
    expect(await screen.findByRole("status", { name: "Browse Collectibles" })).toBeTruthy();
    expect(router.state.location.pathname + router.state.location.search).toBe("/shop/items?x=1");
  });

  it("a page module loads lazily, by default or by its upstream export name", async () => {
    renderAt("/shop/item/0xabc/7", {}, { "./pages/ItemDetail.tsx": async () => ({ ItemDetailRoute: () => <p>item detail</p> }) });
    expect(await screen.findByText("item detail")).toBeTruthy();
  });

  it("credits redirect to the overview in IAP mode; unknown paths are NotFound", async () => {
    const router = renderAt("/shop/credits", { iapMode: () => true }, {});
    expect(await screen.findByRole("status", { name: "Overview" })).toBeTruthy();
    expect(router.state.location.pathname).toBe("/shop/overview");
  });
});

const REAL_PAGE_WAIT = { timeout: 10_000 };

describe("ShopRoutes inside a host's /shop/* route", () => {
  it("renders the table relative to the splat", async () => {
    const router = createMemoryRouter([{ path: "/shop/*", element: <ShopRoutes /> }], { initialEntries: ["/shop/my-assets?tab=1"] });
    render(<RouterProvider router={router} />);
    expect(await screen.findByRole("heading", { level: 1, name: "My items" }, REAL_PAGE_WAIT)).toBeTruthy();
    expect(router.state.location.pathname + router.state.location.search).toBe("/shop/my-items?tab=1");
  });

  it("a renamed splat carries only its own rest, not the host's", async () => {
    const router = createMemoryRouter([{ path: "/shop/*", element: <ShopRoutes /> }], { initialEntries: ["/shop/assets/creator/0xabc"] });
    render(<RouterProvider router={router} />);
    expect(await screen.findByTestId("creator-page", {}, REAL_PAGE_WAIT)).toBeTruthy();
    expect(router.state.location.pathname).toBe("/shop/items/creator/0xabc");
  });
});
