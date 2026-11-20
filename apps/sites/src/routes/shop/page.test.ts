import { afterEach, describe, expect, it } from "vitest";

import { shopFlags } from "@ui/shop/discovery/flags";
import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";

import { vi } from "vitest";

import { DEFAULT_SOCIAL_IMAGE } from "../lib/social-meta";
import { primeShopPageData, shopClientLoader, shopMeta } from "./page";

afterEach(() => {
  primeRemoteFlags(null);
  vi.unstubAllGlobals();
});

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

  it("primes them for a Shop reached by a client navigation, before the page renders", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ "dapps-shop-creator-sales": true }))));
    expect(shopFlags().creatorSales).toBe(false);
    expect(await shopClientLoader()).toBeNull();
    expect(shopFlags().creatorSales).toBe(true);
  });
});

describe("a Shop page's head", () => {
  const C = "0xabc0000000000000000000000000000000000001";
  const matches = [{ id: "root", loaderData: { publicPageBase: "https://dcl.test" } }];
  type Tags = ReturnType<ReturnType<typeof shopMeta>>;
  const content = (tags: Tags, key: string) =>
    tags.filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key)).map((t) => (t as { content: string }).content);
  const data = (page: "itemDetail" | "tokenDetail" | "overview" | "cart", seo: object) => ({ page, seo: { description: "A hat.", noindex: false, ...seo }, queries: [] });

  it("an item with a thumbnail is a small product card under the Shop's name, canonical to the item", () => {
    const canonical = `https://dcl.test/shop/item/${C}/7`;
    const tags = shopMeta("itemDetail")({ loaderData: data("itemDetail", { title: "Pirate Hat", image: "https://img.test/7.png", canonical }), matches });
    expect(tags[0]).toEqual({ title: "Pirate Hat | Decentraland Shop" });
    expect(content(tags, "og:title")).toEqual(["Pirate Hat | Decentraland Shop"]);
    expect(content(tags, "twitter:title")).toEqual(["Pirate Hat | Decentraland Shop"]);
    expect(content(tags, "description")).toEqual(["A hat."]);
    expect(content(tags, "twitter:description")).toEqual(["A hat."]);
    expect(content(tags, "og:image")).toEqual(["https://img.test/7.png"]);
    expect(content(tags, "twitter:image")).toEqual(["https://img.test/7.png"]);
    expect(content(tags, "twitter:card")).toEqual(["summary"]);
    expect(content(tags, "twitter:site")).toEqual(["@decentraland"]);
    expect(content(tags, "twitter:creator")).toEqual(["@decentraland"]);
    expect(content(tags, "og:type")).toEqual(["product"]);
    expect(content(tags, "og:site_name")).toEqual(["Decentraland Shop"]);
    expect(content(tags, "og:locale")).toEqual(["en_US"]);
    expect(content(tags, "og:url")).toEqual([canonical]);
    expect(tags).toContainEqual({ tagName: "link", rel: "canonical", href: canonical });
    expect(content(tags, "robots")).toEqual([]);
    expect(tags.some((t) => "script:ld+json" in t)).toBe(false);
  });

  it("an item or token without an image shows the shared image as a large card", () => {
    for (const page of ["itemDetail", "tokenDetail"] as const) {
      const tags = shopMeta(page)({ loaderData: data(page, { title: "Pirate Hat" }), matches });
      expect(content(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(content(tags, "og:type")).toEqual(["product"]);
    }
  });

  it("a token with an image is small, and a LAND token's map is large", () => {
    const token = shopMeta("tokenDetail")({ loaderData: data("tokenDetail", { title: "Pirate Hat #5", image: "https://img.test/5.png" }), matches });
    expect(content(token, "twitter:card")).toEqual(["summary"]);
    const land = shopMeta("tokenDetail")({ loaderData: data("tokenDetail", { title: "Parcel", image: "https://img.test/map.png", largeImage: true, noindex: true }), matches });
    expect(content(land, "og:image")).toEqual(["https://img.test/map.png"]);
    expect(content(land, "twitter:card")).toEqual(["summary_large_image"]);
  });

  it("a private page stays out of the index with no canonical", () => {
    const tags = shopMeta("cart")({ loaderData: null, matches });
    expect(content(tags, "robots")).toEqual(["noindex"]);
    expect(content(tags, "og:url")).toEqual([]);
    expect(tags.some((t) => "rel" in t)).toBe(false);
    expect(content(tags, "og:type")).toEqual(["website"]);
  });

  it("a public page's canonical is the loader's, and the event grid is never indexed", () => {
    expect(shopMeta("items")({ loaderData: { page: "items", seo: { title: "Items", description: "x", noindex: false, canonical: "https://x.test/shop/items" }, queries: [] } })).toContainEqual({
      tagName: "link",
      rel: "canonical",
      href: "https://x.test/shop/items",
    });
    const event = shopMeta("event")({ loaderData: { page: "event", seo: { title: "Event", description: "x", noindex: false, canonical: "https://x.test/shop/event" }, queries: [] } });
    expect(content(event, "robots")).toEqual(["noindex"]);
    expect(event.some((t) => "rel" in t)).toBe(false);
  });

  it("the overview keeps the Shop's default title and describes the Shop as a site", () => {
    const tags = shopMeta("overview")({ loaderData: data("overview", { title: "Shop", canonical: "https://dcl.test/shop/overview" }), matches });
    expect(tags[0]).toEqual({ title: "Decentraland Shop | Wearables & Emotes for Your Avatar" });
    expect(content(tags, "og:type")).toEqual(["website"]);
    expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
    expect(tags).toContainEqual({
      "script:ld+json": {
        "@context": "https://schema.org",
        "@type": "WebSite",
        name: "Decentraland Shop",
        url: "https://dcl.test/shop/overview",
        description: "A hat.",
        isPartOf: { "@type": "WebSite", name: "Decentraland", url: "https://dcl.test/" },
        publisher: { "@type": "Organization", name: "Decentraland", url: "https://dcl.test/" },
      },
    });
  });
});
