
import { afterEach, describe, expect, it } from "vitest";

import { createShopClient } from "../api/shop";
import {
  fetchClassicItemOrders,
  fetchItemMeta,
  fetchItemResales,
  fetchOwnedItemCount,
  fetchTokenById,
  fetchUnifiedListingForItem,
  pickItemListing,
} from "../detail/api";
import {
  compactEquipped,
  fetchCatalogByIds,
  fetchCatalogItems,
  fetchCollection,
  fetchShopItems,
  fetchSuggestions,
  fetchTrendingItems,
  sanitizeCollectionName,
  shopListingToItem,
  unifiedListingToItem,
  withoutSale,
} from "./api";
import { catalogClient, catalogTransport, COLLECTION, CREATOR, ITEM_ID, PRIMARY_WEARABLE, RESALE_WEARABLE, RESELLER, saleWearable, STORE_EMOTE, TOKEN_ID } from "./fixtures";
import { indexUnavailable, needsNotice, readFreshness, readMirrorFreshness } from "./freshness";

type FlagWindow = Window & { __SHOP_FLAGS__?: { creatorSales?: boolean; secondarySales?: boolean } };

afterEach(() => {
  delete (window as FlagWindow).__SHOP_FLAGS__;
});

describe("the browse feed", () => {
  it("asks the canonical unified route for one card per item, with upstream's parameters", async () => {
    const client = catalogClient();
    const page = await fetchShopItems(
      { first: 48, skip: 48, category: "wearable", rarities: ["epic", "rare"], wearableCategories: ["hat"], minPriceCredits: 5, search: "hat", sortBy: "cheapest", onSale: true, listingType: "primary", contractAddresses: [] },
      { client },
    );
    const url = new URL(client.calls[0]!.url);
    expect(url.pathname).toBe("/shop-api/v3/catalog/unified");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      category: "wearable",
      first: "48",
      skip: "48",
      contractAddress: "0x0000000000000000000000000000000000000000",
      rarity: "epic,rare",
      wearableCategory: "hat",
      minPriceCredits: "5",
      search: "hat",
      sortBy: "cheapest",
      onSale: "true",
      listingType: "primary",
      groupBy: "item",
    });
    expect(page.items.map((i) => i.listingCount)).toContain(2);
    expect(client.calls[0]!.signed).toBe(false);
  });

  it("goes to the /marketplace-api alias in legacy mode, same query", async () => {
    const transport = catalogTransport();
    const client = createShopClient({ base: "http://shop.test", mode: "legacy", transport, signer: null });
    await fetchTrendingItems({ first: 12, listingType: "primary" }, { client });
    const url = new URL(transport.calls[0]!.url);
    expect(url.pathname).toBe("/marketplace-api/v3/catalog/trending");
    expect(url.searchParams.get("includeSocialEmotes")).toBe("false");
  });

  it("maps a unified row: seconds to ms, a store mint keeps its MANA, a resale its token", () => {
    (window as FlagWindow).__SHOP_FLAGS__ = { creatorSales: true };
    const sale = unifiedListingToItem(saleWearable(1_000_000_000_000));
    expect(sale.saleEndsAt).toBe((1_000_000_000 + 86_400) * 1000);
    expect(sale.compareAtCredits).toBe(20);
    const mint = unifiedListingToItem(STORE_EMOTE);
    expect(mint).toMatchObject({ acquisition: "store", manaWei: "4020000000000000000", tradeId: undefined, id: `${COLLECTION}-8` });
    const resale = unifiedListingToItem(RESALE_WEARABLE);
    expect(resale).toMatchObject({ tokenId: TOKEN_ID, listingType: "secondary", seller: RESELLER, available: undefined });
  });

  it("with creator sales off (the default), erases the discount from the whole row, coupon included", () => {
    const raw = { ...saleWearable(), coupon: { id: "c" } as never };
    expect(withoutSale(raw)).toMatchObject({ priceCredits: 20, compareAtCredits: null, saleEndsAt: null, coupon: null });
    expect(shopListingToItem(raw)).toMatchObject({ priceCredits: 20, compareAtCredits: undefined, coupon: undefined });
  });

  it("carries the freshness the answer reports", async () => {
    const client = catalogClient("ok", { "/v3/catalog/unified": { data: [PRIMARY_WEARABLE], total: 1, freshness: { completedAt: "2026-09-25T10:00:00Z", complete: false, source: "tradesMirror" } } });
    const page = await fetchShopItems({}, { client });
    expect(page.meta.freshness).toEqual({ kind: "fresh", asOf: Date.parse("2026-09-25T10:00:00Z"), lagSeconds: null, complete: false, source: "tradesMirror" });
  });

  it("keeps the 503 unavailable envelope as an index that is catching up", async () => {
    const error = await fetchShopItems({}, { client: catalogClient("outage") }).catch((e) => e);
    expect(indexUnavailable(error)).toEqual({ kind: "unavailable", source: "itemsMirror", message: "the items mirror has no complete pass yet", configure: "MARKETPLACE_MIRRORS_ENABLED" });
    const other = await fetchShopItems({}, { client: catalogClient("error") }).catch((e) => e);
    expect(indexUnavailable(other)).toBeNull();
  });
});

describe("the catalogue and collections", () => {
  it("asks /v3/catalog/items with repeated rarities and an empty set as the zero address", async () => {
    const client = catalogClient();
    await fetchCatalogItems({ contractAddresses: [], rarities: ["epic", "rare"], isOnSale: false }, { client });
    const url = new URL(client.calls[0]!.url);
    expect(url.pathname).toBe("/shop-api/v3/catalog/items");
    expect(url.searchParams.getAll("rarity")).toEqual(["epic", "rare"]);
    expect(url.searchParams.getAll("contractAddress")).toEqual(["0x0000000000000000000000000000000000000000"]);
    expect(url.searchParams.get("isOnSale")).toBe("false");
  });

  it("returns catalogue rows by id in the ids' order, dropping unknown ones", async () => {
    const client = catalogClient();
    const items = await fetchCatalogByIds([`${COLLECTION}-3`, "0xdead-1", `${COLLECTION}-7`], { client });
    expect(items.map((i) => i.itemId)).toEqual(["3", "7"]);
    expect(new URL(client.calls[0]!.url).searchParams.getAll("id")).toHaveLength(3);
  });

  it("reads the collection's name and creator, sanitising a dcl:// name", async () => {
    expect(await fetchCollection(COLLECTION, { client: catalogClient() })).toMatchObject({ name: "Nacho Collection", creator: CREATOR });
    expect(sanitizeCollectionName("dcl://halloween_2019")).toBe("Halloween 2019");
  });

  it("answers the search box's three sections in one request", async () => {
    const client = catalogClient();
    const s = await fetchSuggestions("  nacho ", {}, { client });
    expect(new URL(client.calls[0]!.url).searchParams.get("search")).toBe("nacho");
    expect(s.items[0]).toMatchObject({ name: "Nacho Hat", creatorName: "Nacho" });
    expect(s.collections[0]!.name).toBe("Nacho Collection");
    expect(s.creators[0]!.address).toBe(CREATOR);
    expect(await fetchSuggestions("   ", {}, { client })).toEqual({ items: [], total: 0, collections: [], creators: [] });
  });

  it("compacts equipped collections-v2 urns and drops the rest", () => {
    expect(compactEquipped([`urn:decentraland:matic:collections-v2:${COLLECTION.toUpperCase().replace("0X", "0x")}:7`, "urn:decentraland:off-chain:base-avatars:BaseMale"])).toEqual([`${COLLECTION}-7`]);
  });
});

describe("the detail reads", () => {
  it("prefers the creator's mint over a resale, then native over legacy", async () => {
    const listing = await fetchUnifiedListingForItem(COLLECTION, ITEM_ID, { client: catalogClient() });
    expect(listing).toMatchObject({ tradeId: "trade-primary-7", tokenId: undefined });
    const legacy = { ...unifiedListingToItem(PRIMARY_WEARABLE), source: "legacy" as const };
    const native = unifiedListingToItem(PRIMARY_WEARABLE);
    const resale = unifiedListingToItem(RESALE_WEARABLE);
    expect(pickItemListing([resale, legacy, native])).toBe(native);
    expect(pickItemListing([resale])).toBe(resale);
  });

  it("lists only an item's resales (a token and a trade each)", async () => {
    const resales = await fetchItemResales(COLLECTION, ITEM_ID, { client: catalogClient() });
    expect(resales.map((r) => r.tokenId)).toEqual([TOKEN_ID]);
  });

  it("keeps classic orders without a signed trade, with their exact MANA", async () => {
    const client = catalogClient("ok", {
      "/v1/orders": { data: [{ tokenId: "5", issuedId: "5", owner: RESELLER, contractAddress: COLLECTION, price: "3000000000000000000", tradeId: null, expiresAt: 0 }, { tokenId: "6", owner: RESELLER, contractAddress: COLLECTION, price: "1", tradeId: "t" }] },
    });
    expect(await fetchClassicItemOrders(COLLECTION, ITEM_ID, { client })).toEqual([{ tokenId: "5", issuedId: "5", seller: RESELLER, contractAddress: COLLECTION, priceWei: "3000000000000000000", expiresAt: null }]);
  });

  it("maps item metadata: utility trimmed, supply from a string, gender from body shapes", async () => {
    const meta = await fetchItemMeta(COLLECTION, ITEM_ID, { client: catalogClient() });
    expect(meta).toMatchObject({ utility: "Glows at night", available: 10, gender: "unisex", wearableCategory: "hat", creator: CREATOR });
  });

  it("finds a token only when the row matches it, and counts ownership softly", async () => {
    const client = catalogClient();
    expect(await fetchTokenById(COLLECTION, TOKEN_ID, { client })).toMatchObject({ tokenId: TOKEN_ID, owner: RESELLER, isOnSale: true, issuedId: "42" });
    expect(await fetchTokenById(COLLECTION, "999", { client })).toBeNull();
    expect(await fetchOwnedItemCount(RESELLER, COLLECTION, ITEM_ID, { client: catalogClient("error") })).toBe(0);
  });

  it("reads freshness from a body, a nested meta or the mirror status shape", () => {
    expect(readFreshness({ meta: { freshness: { indexedAt: 1_700_000_000, lagSeconds: 30 } } })).toEqual({ kind: "fresh", asOf: 1_700_000_000_000, lagSeconds: 30, complete: true, source: null });
    expect(readFreshness({ completedAt: null, inProgress: true })).toMatchObject({ complete: false });
    expect(readFreshness({ data: [] })).toBeNull();
  });

  it("reads a mirrored copy's as_of as its lag: a notice past the budget its node sends, none before", () => {
    const now = Date.parse("2026-10-02T12:00:00Z");
    const copy = (minutes: number, staleAfterSecs = 600) =>
      readFreshness({ as_of: new Date(now - minutes * 60_000).toISOString(), stale_after_secs: staleAfterSecs, data: [] }, now);
    expect(copy(20)).toEqual({ kind: "fresh", asOf: now - 20 * 60_000, lagSeconds: 1200, complete: true, source: null, staleAfterSeconds: 600 });
    expect(needsNotice(copy(20))).toBe(true);
    expect(needsNotice(copy(4))).toBe(false);
    expect(needsNotice(copy(20, 1800)), "a node with a longer interval").toBe(false);
    expect(needsNotice(copy(40, 1800))).toBe(true);
    expect(readFreshness({ as_of: "2026-10-02T11:00:00Z", freshness: { completedAt: "2026-10-02T11:59:00Z" } }, now)?.asOf).toBe(Date.parse("2026-10-02T11:59:00Z"));
  });

  it("reads the trades mirror's status as the listings' freshness, none when the mirror is off", () => {
    expect(readMirrorFreshness({ ok: true, data: { enabled: true, completedAt: "2026-09-25T10:00:00.000Z", inProgress: true } })).toEqual({
      kind: "fresh",
      asOf: Date.parse("2026-09-25T10:00:00.000Z"),
      lagSeconds: null,
      complete: true,
      source: "tradesMirror",
    });
    expect(readMirrorFreshness({ ok: true, data: { enabled: true, completedAt: null, inProgress: true } })).toMatchObject({ complete: false, asOf: null });
    expect(readMirrorFreshness({ ok: true, data: { enabled: false } })).toBeNull();
  });
});
