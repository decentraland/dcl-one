import { describe, expect, it } from "vitest";

import { catalogClient, catalogTransport, COLLECTION, ITEM_ID, RESALE_WEARABLE } from "../../discovery/fixtures";
import { createShopClient } from "@shop/api/shop";
import { fetchShopListingForItem } from "@shop/detail/api";
import { fetchTradeForItem } from "./api";

/** Creator sales off, as a page has them until the switch says yes. */
const OFF = () => false;

describe("an item line's live trade", () => {
  it("is the creator's primary sale only: an item whose only listing is a resale has none, and no trade is read", async () => {
    const transport = catalogTransport("ok", {
      "/v3/catalog/unified": (url: URL) => {
        const rows = url.searchParams.get("listingType") === "primary" ? [] : [RESALE_WEARABLE];
        return { data: rows, total: rows.length };
      },
    });
    const client = createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: null });
    expect(await fetchTradeForItem(COLLECTION, ITEM_ID, client)).toBeNull();
    const asked = transport.calls.map((c) => new URL(c.url, "http://shop.test"));
    expect(asked.filter((u) => u.pathname.endsWith("/v3/catalog/unified")).every((u) => u.searchParams.get("listingType") === "primary")).toBe(true);
    expect(asked.some((u) => u.pathname.includes("/v1/trades"))).toBe(false);
  });

  it("never takes a row naming a token, even if a primary read returned one", async () => {
    const client = catalogClient("ok", { "/v3/catalog/unified": { data: [RESALE_WEARABLE], total: 1 } });
    expect(await fetchTradeForItem(COLLECTION, ITEM_ID, client)).toBeNull();
  });
});

describe("the in-world shell's item read", () => {
  it("asks for the creator's listing only, and never takes a resale for the item", async () => {
    const transport = catalogTransport("ok", {
      "/v3/catalog/shop": (url: URL) => ({ data: url.searchParams.get("listingType") === "primary" ? [] : [RESALE_WEARABLE], total: 1 }),
    });
    const client = createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: null });
    expect(await fetchShopListingForItem(COLLECTION, ITEM_ID, { client, creatorSales: OFF })).toBeNull();
    const asked = transport.calls.map((c) => new URL(c.url, "http://shop.test")).filter((u) => u.pathname.endsWith("/v3/catalog/shop"));
    expect(asked.length).toBeGreaterThan(0);
    expect(asked.every((u) => u.searchParams.get("listingType") === "primary")).toBe(true);
    const leaky = catalogClient("ok", { "/v3/catalog/shop": { data: [RESALE_WEARABLE], total: 1 } });
    expect(await fetchShopListingForItem(COLLECTION, ITEM_ID, { client: leaky, creatorSales: OFF })).toBeNull();
  });
});
