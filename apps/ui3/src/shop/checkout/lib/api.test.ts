import { describe, expect, it } from "vitest";

import { catalogClient, catalogTransport, COLLECTION, ITEM_ID, PRIMARY_WEARABLE, RESALE_WEARABLE, STORE_EMOTE } from "../../discovery/fixtures";
import { createShopClient } from "@shop/api/shop";
import { fetchStoreMintState, fetchTradeForItem } from "./api";

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

describe("a store mint's live state", () => {
  /** The same item's creator trade in MANA, listed after the mint was created: newest first, it comes before the mint. */
  const LEGACY_TRADE = { ...PRIMARY_WEARABLE, source: "legacy" as const, tradeId: "trade-legacy-mint-item", itemId: STORE_EMOTE.itemId, manaWei: "1000000000000000000" };

  it("is found among the item's legacy rows, even when its creator's MANA trade comes first", async () => {
    const transport = catalogTransport("ok", {
      "/v3/catalog/unified": (url: URL) => ({ data: url.searchParams.get("source") === "legacy" ? [LEGACY_TRADE, STORE_EMOTE] : [LEGACY_TRADE], total: 2 }),
    });
    const client = createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: null });
    expect(await fetchStoreMintState(COLLECTION, STORE_EMOTE.itemId!, client)).toEqual({ priceWei: STORE_EMOTE.manaWei, available: STORE_EMOTE.available });
  });

  it("is found past the first page, after more MANA trades than a page holds", async () => {
    const rows = [...Array.from({ length: 20 }, (_, i) => ({ ...LEGACY_TRADE, tradeId: `trade-legacy-${i}` })), STORE_EMOTE];
    const transport = catalogTransport("ok", {
      "/v3/catalog/unified": (url: URL) => {
        const skip = Number(url.searchParams.get("skip") ?? 0);
        const first = Number(url.searchParams.get("first") ?? rows.length);
        return { data: rows.slice(skip, skip + first), total: rows.length };
      },
    });
    const client = createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: null });
    expect(await fetchStoreMintState(COLLECTION, STORE_EMOTE.itemId!, client)).toEqual({ priceWei: STORE_EMOTE.manaWei, available: STORE_EMOTE.available });
    const skips = transport.calls.map((c) => new URL(c.url, "http://shop.test").searchParams.get("skip"));
    expect(skips).toEqual(["0", "20"]);
  });

  it("is null when the item has no store mint any more", async () => {
    const client = catalogClient("ok", { "/v3/catalog/unified": { data: [LEGACY_TRADE], total: 1 } });
    expect(await fetchStoreMintState(COLLECTION, STORE_EMOTE.itemId!, client)).toBeNull();
  });
});
