import { afterEach, describe, expect, it, vi } from "vitest";

import { CHAIN } from "../../model/chain";
import { BOOK } from "../../payment/fixtures";
import { LISTS, MANA, OWNED_NAMES, OWNED_WEARABLES, RECEIVED_BIDS, MADE_BIDS, SALES, list } from "../fixtures";
import { fakeShop } from "../testing";
import { dataUnavailable, fetchBidsByAddress, fetchMyAssets, fetchOwnedToken, fetchUserSales, fetchVolume, fetchRankings } from "./api";
import {
  bulkPick,
  canEditList,
  canViewList,
  createList,
  DEFAULT_LIST_ID,
  deleteList,
  favoriteKey,
  fetchFavoriteIds,
  fetchFavoriteStats,
  fetchListPicks,
  getList,
  ListNotAvailableError,
  validateListDraft,
} from "./favorites";

afterEach(() => {
  vi.useRealTimers();
});

describe("list privacy: a private list is never rendered to another account", () => {
  const privateList = LISTS[1]!;
  const sharedList = LISTS[2]!;

  it("canViewList: the owner and grantees only for a private list; anyone for a shared one", () => {
    expect(canViewList(privateList, BOOK.buyer)).toBe(true);
    expect(canViewList(privateList, BOOK.reseller)).toBe(false);
    expect(canViewList(privateList, null)).toBe(false);
    expect(canViewList({ ...privateList, permission: "view" }, BOOK.reseller)).toBe(true);
    expect(canViewList(sharedList, null)).toBe(true);
    expect(canViewList(LISTS[0]!, null)).toBe(false);
    expect(canEditList(privateList, BOOK.reseller)).toBe(false);
    expect(canEditList({ ...privateList, permission: "edit" }, BOOK.reseller)).toBe(true);
    expect(canEditList(LISTS[0]!, BOOK.buyer)).toBe(false);
  });

  it("getList refuses a private list the server leaked to a stranger, and picks are never fetched for it", async () => {
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/lists\/list-private$/, reply: () => ({ body: { ok: true, data: privateList } }) },
      { method: "GET", path: /^\/v1\/lists\/list-private\/picks$/, reply: () => ({ body: { ok: true, data: { results: [{ itemId: "x", createdAt: 1 }], total: 1 } } }) },
    ]);
    await expect(getList("list-private", BOOK.reseller, shop.client)).rejects.toBeInstanceOf(ListNotAvailableError);
    await expect(fetchListPicks(privateList, BOOK.reseller, {}, shop.client)).rejects.toBeInstanceOf(ListNotAvailableError);
    expect(shop.requests.some((r) => r.url.pathname.endsWith("/picks"))).toBe(false);
    expect(await getList("list-private", BOOK.buyer, shop.client)).toEqual(privateList);
  });

  it("without the single-list route (v2 today) the owner still gets their own list, from their signed lists", async () => {
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/lists\/list-private$/, reply: () => ({ status: 404, body: { error: "Not Found" } }) },
      { method: "GET", path: /^\/v1\/lists$/, reply: () => ({ body: { ok: true, data: { results: LISTS, total: 3 } } }) },
    ]);
    expect(await getList("list-private", BOOK.buyer, shop.client)).toEqual(LISTS[1]);
    await expect(getList("list-private", BOOK.reseller, shop.client)).rejects.toBeInstanceOf(ListNotAvailableError);
    await expect(getList("list-private", null, shop.client)).rejects.toBeInstanceOf(ListNotAvailableError);
  });

  it("a 403 and a 404 give the same answer, so a probe cannot tell a private list from none", async () => {
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/lists\/forbidden$/, reply: () => ({ status: 403, body: { ok: false, message: "forbidden" } }) },
      { method: "GET", path: /^\/v1\/lists\/missing$/, reply: () => ({ status: 404, body: { ok: false, message: "not found" } }) },
    ]);
    const a = await getList("forbidden", BOOK.reseller, shop.client).catch((e: Error) => e);
    const b = await getList("missing", BOOK.reseller, shop.client).catch((e: Error) => e);
    expect(a).toBeInstanceOf(ListNotAvailableError);
    expect(b).toBeInstanceOf(ListNotAvailableError);
    expect((a as Error).message).toBe((b as Error).message);
  });
});

describe("favorites and lists over /shop-api", () => {
  it("keys an item by contract-itemId and never by a trade id", () => {
    expect(favoriteKey({ contractAddress: "0xABC", itemId: "7" })).toBe("0xabc-7");
    expect(favoriteKey({ contractAddress: "0xabc", itemId: null })).toBeNull();
  });

  it("pages the default list's picks until exhausted", async () => {
    const shop = fakeShop([
      {
        method: "GET",
        path: /^\/v1\/lists\/[^/]+\/picks$/,
        reply: ({ url }) => {
          const offset = Number(url.searchParams.get("offset"));
          const results = offset === 0 ? Array.from({ length: 100 }, (_, i) => ({ itemId: `a-${i}`, createdAt: i })) : [{ itemId: "a-100", createdAt: 100 }];
          return { body: { ok: true, data: { results, total: 101 } } };
        },
      },
    ]);
    const ids = await fetchFavoriteIds(shop.client);
    expect(ids).toHaveLength(101);
    expect(shop.requests[0]!.url.pathname).toBe(`/shop-api/v1/lists/${DEFAULT_LIST_ID}/picks`);
    expect(shop.requests[0]!.signed).toBe(true);
  });

  it("bulk pick sends pickedFor/unpickedFrom only when non-empty; create sends `private`", async () => {
    const shop = fakeShop([
      { method: "POST", path: /^\/v1\/picks\//, reply: () => ({ body: { ok: true, data: { pickedByUser: true } } }) },
      { method: "POST", path: /^\/v1\/lists$/, reply: ({ body }) => ({ body: { ok: true, data: list({ id: "new", name: (body as { name: string }).name }) } }) },
    ]);
    await bulkPick("0xabc-7", ["list-a"], [], shop.client);
    expect(shop.requests[0]!.body).toEqual({ pickedFor: ["list-a"] });
    await createList({ name: "  Summer  ", isPrivate: false, description: "hi" }, shop.client);
    expect(shop.requests[1]!.body).toEqual({ name: "Summer", private: false, description: "hi" });
    await expect(deleteList(DEFAULT_LIST_ID, shop.client)).rejects.toThrow(/default/);
    expect(validateListDraft({ name: " ", isPrivate: true })).toBe("nameRequired");
    expect(validateListDraft({ name: "x".repeat(33), isPrivate: true })).toBe("nameTooLong");
    expect(validateListDraft({ name: "ok", description: "y".repeat(101), isPrivate: true })).toBe("descriptionTooLong");
  });

  it("coalesces stats asks into one bulk read and reads a missing row as zero saves", async () => {
    const shop = fakeShop([
      {
        method: "GET",
        path: /^\/v1\/picks\/stats$/,
        reply: ({ url }) => ({
          body: { ok: true, data: url.searchParams.getAll("itemId").includes("k1") ? [{ itemId: "k1", count: 4, pickedByUser: true }] : [] },
        }),
      },
    ]);
    const [a, b] = await Promise.all([fetchFavoriteStats("k1", BOOK.buyer, shop.client), fetchFavoriteStats("k2", BOOK.buyer, shop.client)]);
    expect(a).toEqual({ count: 4, pickedByUser: true });
    expect(b).toEqual({ count: 0, pickedByUser: false });
    expect(shop.requests).toHaveLength(1);
    expect(shop.requests[0]!.url.searchParams.get("checkingUserAddress")).toBe(BOOK.buyer);
  });
});

describe("owned assets and sales", () => {
  it("maps an owned token's open listing to its signed MANA price, never a credit price", async () => {
    const shop = fakeShop([{ method: "GET", path: /^\/v1\/nfts$/, reply: () => ({ body: { data: OWNED_WEARABLES, total: 3 } }) }]);
    const { assets, total } = await fetchMyAssets(BOOK.buyer, { category: "wearable", onlyOnSale: true, rarities: ["rare"] }, shop.client);
    expect(total).toBe(3);
    expect(assets[0]!.listing?.price).toEqual({ kind: "mana", wei: BigInt(MANA("12.5")) });
    expect(assets[0]).not.toHaveProperty("listingPrice");
    const q = shop.requests[0]!.url.searchParams;
    expect(q.get("owner")).toBe(BOOK.buyer);
    expect(q.get("isOnSale")).toBe("true");
    expect(q.getAll("itemRarity")).toEqual(["rare"]);
  });

  it("claims ownership only of the exact token the owner holds", async () => {
    const shop = fakeShop([{ method: "GET", path: /^\/v1\/nfts$/, reply: ({ url }) => ({ body: { data: url.searchParams.get("tokenId") === "42" ? OWNED_NAMES : [], total: 1 } }) }]);
    expect(await fetchOwnedToken(BOOK.buyer, BOOK.registrar.contract, "42", shop.client)).toMatchObject({ tokenId: "42", category: "ens" });
    expect(await fetchOwnedToken(BOOK.reseller, BOOK.registrar.contract, "42", shop.client)).toBeNull();
    expect(await fetchOwnedToken(BOOK.buyer, BOOK.registrar.contract, "43", shop.client)).toBeNull();
  });

  it("reads sales by role and bids made and received", async () => {
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/sales$/, reply: () => ({ body: { data: SALES, total: 3 } }) },
      {
        method: "GET",
        path: /^\/v1\/bids$/,
        reply: ({ url }) => ({ body: { ok: true, data: { results: url.searchParams.get("bidder") ? MADE_BIDS : RECEIVED_BIDS, total: 2, page: 0, pages: 1, limit: 1000 } } }),
      },
    ]);
    const sales = await fetchUserSales(BOOK.creator, { role: "seller" }, shop.client);
    expect(sales.items[0]!.manaWei).toBe(BigInt(MANA(3)));
    expect(shop.requests[0]!.url.searchParams.get("seller")).toBe(BOOK.creator);
    const bids = await fetchBidsByAddress(BOOK.buyer, shop.client);
    expect(bids.received).toHaveLength(2);
    expect(bids.made).toHaveLength(1);
  });

  it("surfaces a 503 unavailable envelope as the missing source, not as an empty page", async () => {
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/volume\/all$/, reply: () => ({ status: 503, body: { error: "Service Unavailable", message: "timeframe all needs the nft indexer", source: "nft-indexer" } }) },
      { method: "GET", path: /^\/v1\/rankings\/creators\/week$/, reply: () => ({ body: { data: [{ id: BOOK.creator, sales: 1, earned: "1", collections: 1, uniqueCollectors: 1 }] } }) },
    ]);
    const err = await fetchVolume("all", shop.client).catch((e: unknown) => e);
    expect(dataUnavailable(err)).toEqual({ source: "nft-indexer", message: "timeframe all needs the nft indexer", configure: null });
    expect(await fetchRankings("creators", "week", {}, shop.client)).toHaveLength(1);
    expect(shop.requests[1]!.url.searchParams.get("first")).toBe("5");
  });
});

describe("chains", () => {
  it("fixtures carry Polygon collections and Ethereum NAMEs", () => {
    expect(OWNED_WEARABLES[0]!.nft.chainId).toBe(CHAIN.POLYGON);
    expect(OWNED_NAMES[0]!.nft.chainId).toBe(CHAIN.ETHEREUM);
  });
});
