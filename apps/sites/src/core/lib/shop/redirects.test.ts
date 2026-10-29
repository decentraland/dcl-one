import { describe, expect, it } from "vitest";

import { landTokenId } from "@ui/data/catalyst/landListing";

import { assetPage, isTokenRegistry, legacyShopTarget, marketplaceTarget, parseAssetRef, shopIndexTarget } from "./redirects";

const C = "0xabc0000000000000000000000000000000000001";
const LAND = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
const ESTATE = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
const REGISTRAR = "0x2a187453064356c898cae034eaed119e1663acb8";
const ME = "0x1111111111111111111111111111111111111111";

/** A target as path + query map, so parameter order never decides a test. */
function at(target: string | null): { path: string; query: Record<string, string> } {
  const url = new URL(target ?? "/null", "https://sites.test");
  return { path: url.pathname, query: Object.fromEntries(url.searchParams) };
}

function mk(url: string) {
  const u = new URL(url, "https://sites.test");
  return at(marketplaceTarget(u.pathname, u.search, u.hash));
}

describe("asset identity", () => {
  it("reads v2's contract-id refs in either spelling", () => {
    expect(parseAssetRef(`${C.toUpperCase().replace("0X", "0x")}-3`)).toEqual({ contract: C, id: "3" });
    expect(parseAssetRef(`${C}:3`)).toEqual({ contract: C, id: "3" });
    expect(parseAssetRef("0xdead:1")).toBeNull();
    expect(parseAssetRef(`${C}-`)).toBeNull();
  });

  it("a LAND, estate or NAME registrar id is a token; a collection's id is an item", () => {
    expect([isTokenRegistry(LAND), isTokenRegistry(ESTATE), isTokenRegistry(REGISTRAR), isTokenRegistry(C)]).toEqual([true, true, true, false]);
    expect(assetPage(C, "3")).toBe(`/shop/item/${C}/3`);
    expect(assetPage(LAND, "42")).toBe(`/shop/token/${LAND}/42`);
    expect(assetPage(C, "9", "token")).toBe(`/shop/token/${C}/9`);
  });
});

describe("v2's /marketplace pages", () => {
  it("/marketplace is the overview; in the iOS IAP view, the items (marker kept)", () => {
    expect(mk("/marketplace?utm_source=x")).toEqual({ path: "/shop/overview", query: { utm_source: "x" } });
    expect(mk("/marketplace?view=mobile-iap&section=wearables")).toEqual({ path: "/shop/items", query: { view: "mobile-iap", category: "wearable" } });
  });

  it("/marketplace/:id opens the item (or the registry token) it names, the notification's tracking kept", () => {
    expect(mk(`/marketplace/${C}-3?src=ffw&nid=n1`)).toEqual({ path: `/shop/item/${C}/3`, query: { src: "ffw", nid: "n1" } });
    expect(mk(`/marketplace/${encodeURIComponent(`${C}:3`)}`).path).toBe(`/shop/item/${C}/3`);
    expect(mk(`/marketplace/${LAND}-42`).path).toBe(`/shop/token/${LAND}/42`);
  });

  it("buy, mint and express checkout land on the asset page where it is bought", () => {
    expect(mk(`/marketplace/buy?item=${C}-3`).path).toBe(`/shop/item/${C}/3`);
    expect(mk(`/marketplace/buy?nft=${C}-77`).path).toBe(`/shop/token/${C}/77`);
    expect(mk(`/marketplace/mint?item=${C}-3`).path).toBe(`/shop/item/${C}/3`);
    expect(mk(`/marketplace/checkout?express=${C}-3&state=review`)).toEqual({ path: `/shop/item/${C}/3`, query: {} });
    expect(mk("/marketplace/checkout?tour=1").path).toBe("/shop/cart");
    expect(mk("/marketplace/cart").path).toBe("/shop/cart");
    expect(mk("/marketplace/buy").path).toBe("/shop/items");
  });

  it("the token actions open the token's action page, else where the owner picks the token", () => {
    expect(mk(`/marketplace/sell?nft=${C}-9&redirectTo=%2Fshop%2Fmy-items`)).toEqual({ path: `/shop/token/${C}/9/sell`, query: { redirectTo: "/shop/my-items" } });
    expect(mk(`/marketplace/sell?wallet=${ME}&step=price`)).toEqual({ path: "/shop/my-items", query: {} });
    expect(mk("/marketplace/transfer").path).toBe("/shop/my-items");
    expect(mk("/marketplace/cancel?order=12")).toEqual({ path: "/shop/my-store", query: { order: "12" } });
    expect(mk(`/marketplace/cancel?order=12&owner=${ME}&ownership=self`)).toEqual({ path: "/shop/my-store", query: { order: "12" } });
    expect(mk(`/marketplace/cancel?contractAddress=${C}&tokenId=9`).path).toBe(`/shop/token/${C}/9/cancel`);
    expect(mk("/marketplace/accept-bid?bid=b1")).toEqual({ path: "/shop/my-items", query: { section: "bids", bid: "b1" } });
    expect(mk(`/marketplace/accept-bid?nft=${C}-9&bid=b1`)).toEqual({ path: `/shop/token/${C}/9/bids`, query: { bid: "b1" } });
    expect(mk(`/marketplace/manage?id=${ESTATE}-7&owner=${ME}`).path).toBe(`/shop/token/${ESTATE}/7/manage`);
    expect(mk(`/marketplace/bid?id=${C}-3&step=amount`)).toEqual({ path: `/shop/item/${C}/3/bid`, query: {} });
  });

  it("account, lists, settings, activity, credits and NAMEs", () => {
    expect(mk("/marketplace/account").path).toBe("/shop/my-items");
    expect(mk("/marketplace/account?tab=on-sale").path).toBe("/shop/my-store");
    expect(mk("/marketplace/account?tab=bids")).toEqual({ path: "/shop/my-items", query: { section: "bids" } });
    expect(mk(`/marketplace/account?address=${ME.toUpperCase().replace("0X", "0x")}&tab=collections`).path).toBe(`/shop/accounts/${ME}`);
    expect(mk("/marketplace/lists")).toEqual({ path: "/shop/my-favorites", query: { tab: "lists" } });
    expect(mk("/marketplace/lists?listId=l-1").path).toBe("/shop/my-favorites/l-1");
    expect(mk("/marketplace/settings").path).toBe("/shop/settings");
    expect(mk("/marketplace/settings?tab=store").path).toBe("/shop/store-settings");
    expect(mk("/marketplace/activity?type=bid")).toEqual({ path: "/shop/activity", query: { type: "bid" } });
    expect(mk("/marketplace/credits?step=packs&granted=1&order=o1")).toEqual({ path: "/shop/credits", query: { order: "o1" } });
    expect(mk("/marketplace/packs").path).toBe("/shop/credits");
    expect(mk("/marketplace/names?search=ana")).toEqual({ path: "/shop/names", query: { search: "ana" } });
    expect(mk("/marketplace/claim-name?name=ana&from=names&step=confirm")).toEqual({ path: "/shop/names", query: { name: "ana", from: "names" } });
  });

  it("browse, shop and collection keep their filters in the Shop's spelling", () => {
    expect(mk("/marketplace/browse?category=emote&rarity=rare&search=wave&sortBy=most_expensive&page=2")).toEqual({
      path: "/shop/items",
      query: { category: "emote", rarities: "rare", q: "wave", sort: "price-desc" },
    });
    expect(mk("/marketplace/shop?tab=all-assets&category=wearable").path).toBe("/shop/items");
    expect(mk(`/marketplace/collection?contract=${C}&sortBy=cheapest`)).toEqual({ path: `/shop/collection/${C}`, query: { sort: "price-asc" } });
    expect(mk("/marketplace/collection").path).toBe("/shop/items");
  });
});

describe("the classic Marketplace's paths (decentraland.org/marketplace)", () => {
  it("asset pages and their actions keep the contract and the id", () => {
    const upper = "0xABC0000000000000000000000000000000000001";
    expect(mk(`/marketplace/contracts/${upper}/tokens/5`).path).toBe(`/shop/token/${C}/5`);
    for (const action of ["sell", "bid", "cancel", "transfer", "manage"]) {
      expect(mk(`/marketplace/contracts/${C}/tokens/5/${action}?redirectTo=%2Faccount`)).toEqual({ path: `/shop/token/${C}/5/${action}`, query: { redirectTo: "/account" } });
    }
    expect(mk(`/marketplace/contracts/${C}/tokens/5/buy?withCard=true`)).toEqual({ path: `/shop/token/${C}/5`, query: { withCard: "true" } });
    expect(mk(`/marketplace/contracts/${C}/tokens/5/buy/status?txHash=0x1`)).toEqual({ path: `/shop/token/${C}/5/buy/status`, query: { txHash: "0x1" } });
    expect(mk(`/marketplace/contracts/${C}/items/3?view=mobile-iap`)).toEqual({ path: `/shop/item/${C}/3`, query: { view: "mobile-iap" } });
    expect(mk(`/marketplace/contracts/${C}/items/3/bid`).path).toBe(`/shop/item/${C}/3/bid`);
    expect(mk(`/marketplace/contracts/${C}/items/3/buy/status`).path).toBe(`/shop/item/${C}/3/buy/status`);
    expect(mk(`/marketplace/collections/${upper}`).path).toBe(`/shop/collection/${C}`);
    expect(mk(`/marketplace/accounts/${ME}?section=on_sale&vendor=decentraland`)).toEqual({ path: `/shop/accounts/${ME}`, query: { section: "on_sale" } });
  });

  it("parcels and estates resolve to their registry token", () => {
    expect(mk("/marketplace/parcels/-10/20/detail").path).toBe(`/shop/token/${LAND}/${landTokenId(-10, 20)}`);
    expect(mk("/marketplace/estates/7/detail").path).toBe(`/shop/token/${ESTATE}/7`);
  });

  it("browse sections map to the Shop's pages and categories", () => {
    expect(mk("/marketplace/browse?section=wearables_hat&rarities=rare_epic&search=cap&sortBy=cheapest&onlyOnSale=true&vendor=decentraland&assetType=item")).toEqual({
      path: "/shop/items",
      query: { category: "wearable", subCategory: "Hat", rarities: "rare,epic", q: "cap", sort: "price-asc", status: "on_sale" },
    });
    expect(mk("/marketplace/browse?section=emotes_dance&onlySmart=true&onlyOnSale=false").query).toEqual({ category: "emote", subCategory: "Dance", smart: "true", status: "all" });
    expect(mk("/marketplace/browse?section=land&onlyOnRent=true")).toEqual({ path: "/shop/lands", query: { section: "land", onlyOnRent: "true" } });
    expect(mk("/marketplace/browse?section=ens")).toEqual({ path: "/shop/names", query: { section: "resale" } });
    expect(mk(`/marketplace/browse?section=wearables&contracts=${C}`)).toEqual({ path: `/shop/collection/${C}`, query: { category: "wearable" } });
    expect(mk("/MARKETPLACE/browse/?section=emotes").path).toBe("/shop/items");
  });

  it("the other pages", () => {
    expect(mk("/marketplace/lands?isMap=false").path).toBe("/shop/lands");
    expect(mk("/marketplace/names/browse")).toEqual({ path: "/shop/names", query: { section: "resale" } });
    expect(mk("/marketplace/names/claim?view=mobile-iap")).toEqual({ path: "/shop/names", query: { view: "mobile-iap" } });
    expect(mk("/marketplace/campaign?section=wearables").path).toBe("/shop/event");
    expect(mk("/marketplace/lists/l-9?page=1")).toEqual({ path: "/shop/my-favorites/l-9", query: { page: "1" } });
    expect(mk("/marketplace/success?txHash=0x1&tokenId=4&assetType=nft&contractAddress=0xa")).toEqual({
      path: "/shop/success",
      query: { txHash: "0x1", tokenId: "4", assetType: "nft", contractAddress: "0xa" },
    });
  });

  it("sign-in returns to the Shop page of where it was going, never to another origin", () => {
    expect(mk("/marketplace/sign-in?redirectTo=%2Faccount").path).toBe("/shop/my-items");
    expect(mk(`/marketplace/sign-in?redirectTo=${encodeURIComponent(`/contracts/${C}/items/3`)}`).path).toBe(`/shop/item/${C}/3`);
    expect(mk("/marketplace/sign-in?redirectTo=%2Fshop%2Fcart").path).toBe("/shop/cart");
    expect(mk("/marketplace/sign-in?redirectTo=%2F%2Fevil.example%2Fx").path).toBe("/shop/my-items");
    expect(mk("/marketplace/sign-in?redirectTo=https%3A%2F%2Fevil.example").path).toBe("/shop/my-items");
  });

  it("an unknown path keeps its place under /shop (a Shop page or the not-found page)", () => {
    expect(mk("/marketplace/overview").path).toBe("/shop/overview");
    expect(mk("/marketplace/credits-email-confirmed/abc?address=0x1").path).toBe("/shop/credits-email-confirmed/abc");
    expect(marketplaceTarget("/marketplaces")).toBeNull();
    expect(marketplaceTarget("/places")).toBeNull();
  });
});

describe("v2's /shop tabs", () => {
  it("each tab opens its page; the rest of the query stays", () => {
    expect(at(shopIndexTarget(""))).toEqual({ path: "/shop/overview", query: {} });
    expect(at(shopIndexTarget("?utm_source=a"))).toEqual({ path: "/shop/overview", query: { utm_source: "a" } });
    expect(at(shopIndexTarget("?tab=all-assets&category=emote&rarity=rare&search=x&sortBy=cheapest&page=3"))).toEqual({
      path: "/shop/items",
      query: { category: "emote", rarities: "rare", q: "x", sort: "price-asc" },
    });
    expect(at(shopIndexTarget("?tab=my-assets")).path).toBe("/shop/my-items");
    expect(at(shopIndexTarget("?tab=my-favorites")).path).toBe("/shop/my-favorites");
    expect(at(shopIndexTarget("?tab=my-lists"))).toEqual({ path: "/shop/my-favorites", query: { tab: "lists" } });
    expect(at(shopIndexTarget("?tab=credits")).path).toBe("/shop/credits");
    expect(at(shopIndexTarget("?tab=cart")).path).toBe("/shop/cart");
    expect(at(shopIndexTarget("?tab=names")).path).toBe("/shop/names");
    expect(at(legacyShopTarget("/shop/", "?tab=cart")).path).toBe("/shop/cart");
  });
});
