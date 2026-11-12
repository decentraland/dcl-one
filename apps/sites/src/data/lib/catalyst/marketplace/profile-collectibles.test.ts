import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../client", async (original) => ({
  ...(await original<typeof import("../client")>()),
  getJSON: vi.fn(),
}));
vi.mock("../warn", () => ({ warnInvalid: vi.fn() }));
vi.mock("./credit-quotes", () => ({ tryQuoteCreditItems: vi.fn() }));

import { CatalystError, getJSON } from "../client";
import { warnInvalid } from "../warn";
import { tryQuoteCreditItems } from "./credit-quotes";
import { CREATIONS_QUOTES_TIMEOUT_MS, fetchProfileAssets, fetchProfileCreations, manaPrice } from "./profile-collectibles";

const CREATOR = "0x09BC69A986E1B37FB01EF36124F4C2E14FA0AB6B";
const CONTRACT = "0x242E85D51F975E0FD3C6208E3E7BE852BC8A11A0";
const MANA = (whole: number) => `${BigInt(whole) * 10n ** 18n}`;

function item(over: Record<string, unknown> = {}) {
  return {
    id: `${CONTRACT}-0`,
    itemId: "0",
    name: " Billyjuice ",
    thumbnail: "https://peer.example/thumbnail",
    contractAddress: CONTRACT,
    rarity: "legendary",
    network: "MATIC",
    chainId: 137,
    category: "wearable",
    price: MANA(369),
    minListingPrice: null,
    isOnSale: true,
    available: 72,
    data: { wearable: { category: "upper_body", bodyShapes: ["BaseMale", "BaseFemale"], isSmart: false } },
    ...over,
  };
}

function token(nft: Record<string, unknown> = {}, order: unknown = null) {
  return {
    nft: {
      id: `${CONTRACT}-54`,
      category: "wearable",
      contractAddress: CONTRACT,
      tokenId: "54",
      name: "Power Up!",
      image: "https://peer.example/image",
      network: "MATIC",
      data: { wearable: { category: "top_head", bodyShapes: ["BaseFemale"], rarity: "epic", isSmart: true } },
      ...nft,
    },
    order,
    rental: null,
  };
}

const json = vi.mocked(getJSON);
const quotes = vi.mocked(tryQuoteCreditItems);
beforeEach(() => quotes.mockImplementation(async (refs) => refs.map(() => "12")));
afterEach(() => {
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe("fetchProfileCreations", () => {
  it("reads one category of the lowercased creator's items, newest first, and maps each card", async () => {
    json.mockResolvedValue({ data: [item()], total: 12 });

    const page = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24, skip: 24 });

    expect(json).toHaveBeenCalledWith("/marketplace-api/v2/catalog", {
      query: { creator: CREATOR.toLowerCase(), category: "wearable", first: 24, skip: 24, sortBy: "newest" },
    });
    expect(page).toEqual({
      total: 12,
      received: 1,
      items: [
        {
          id: `${CONTRACT}-0`,
          contractAddress: CONTRACT.toLowerCase(),
          itemId: "0",
          name: "Billyjuice",
          thumbnail: "https://peer.example/thumbnail",
          rarity: "legendary",
          network: "polygon",
          // A primary Polygon wearable is credit-eligible: priced in credits, as on its Shop card.
          payment: { kind: "credits", credits: "12" },
          traits: { category: "upper_body", bodyShapes: ["BaseMale", "BaseFemale"], isSmart: false },
        },
      ],
    });
    expect(quotes).toHaveBeenCalledWith([{ itemId: "0", collection: CONTRACT.toLowerCase() }], expect.anything());
  });

  it("asks for no quote when the page brings no item", async () => {
    json.mockResolvedValue({ data: [], total: 0 });

    await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(quotes).not.toHaveBeenCalled();
  });

  it("says the credit price is unavailable when the quote fails, never falling back to MANA", async () => {
    quotes.mockResolvedValue([null]);
    json.mockResolvedValue({ data: [item()], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "creditsUnavailable" });
  });

  it("treats a quote that outlasts its deadline as unavailable", async () => {
    vi.useFakeTimers();
    quotes.mockImplementation(() => new Promise(() => {}));
    json.mockResolvedValue({ data: [item()], total: 1 });

    const read = fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });
    await vi.advanceTimersByTimeAsync(CREATIONS_QUOTES_TIMEOUT_MS + 1);

    expect((await read).items[0]?.payment).toEqual({ kind: "creditsUnavailable" });
  });

  it("stops when the visitor leaves while the quote runs", async () => {
    const leave = new AbortController();
    quotes.mockImplementation(() => new Promise(() => {}));
    json.mockResolvedValue({ data: [item()], total: 1 });

    const read = fetchProfileCreations(CREATOR, { category: "wearable", first: 24 }, { signal: leave.signal });
    leave.abort(new Error("left"));

    await expect(read).rejects.toThrow("left");
  });

  it("prices an item that mints on Ethereum in MANA, which credits cannot buy", async () => {
    json.mockResolvedValue({ data: [item({ network: "ETHEREUM", chainId: 1 })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "mana", mana: "369" });
  });

  it("falls back to the cheapest listing when the primary price is zero", async () => {
    json.mockResolvedValue({ data: [item({ price: "0", minListingPrice: MANA(1500) })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "mana", mana: "1,500" });
  });

  it("prices a sold-out primary item exactly as its Shop website card does", async () => {
    // The Shop card's rule (`cardPayment`) does not read `available`; whether a sold-out primary
    // should still show its primary price is an open question for the Shop owner.
    json.mockResolvedValue({ data: [item({ available: 0, minListingPrice: MANA(80) })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "credits", credits: "12" });
  });

  it("gives a delisted item with no listing no price, though the catalogue keeps its old one", async () => {
    json.mockResolvedValue({ data: [item({ isOnSale: false })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "none" });
  });

  it("names an item with a blank name Untitled", async () => {
    json.mockResolvedValue({ data: [item({ name: "  " })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.name).toBe("Untitled");
  });

  it("leaves an item with no sale and no listing without a price", async () => {
    json.mockResolvedValue({ data: [item({ price: "0", minListingPrice: null })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "none" });
  });

  it("keeps an item whose optional facts are missing, without inventing them", async () => {
    json.mockResolvedValue({
      data: [item({ thumbnail: "", rarity: "legendaryish", network: "ETHEREUM", chainId: 1, data: null })],
      total: 1,
    });

    const { items } = await fetchProfileCreations(CREATOR, { category: "emote", first: 24 });

    expect(items[0]).toMatchObject({ thumbnail: undefined, rarity: undefined, network: "ethereum" });
    expect(items[0]?.traits).toEqual({ category: undefined, bodyShapes: [], isSmart: false });
  });

  it("leaves out a row without an identity and reports it", async () => {
    json.mockResolvedValue({ data: [item({ id: undefined }), item({ id: "b", itemId: "1" })], total: 2 });

    const { items, received, total } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items.map((row) => row.id)).toEqual(["b"]);
    expect(total).toBe(2);
    expect(received).toBe(2);
    expect(warnInvalid).toHaveBeenCalledWith("ProfileCreation", expect.anything());
  });

  it("fails on a response without the paged shape instead of showing an empty list", async () => {
    json.mockResolvedValue({ ok: false, error: "404 path not found" });

    await expect(fetchProfileCreations(CREATOR, { category: "wearable", first: 24 })).rejects.toBeInstanceOf(
      CatalystError,
    );
  });

  it("passes a service failure through", async () => {
    json.mockRejectedValue(new CatalystError("Catalyst returned 503", "/marketplace-api/v2/catalog", 503));

    await expect(fetchProfileCreations(CREATOR, { category: "wearable", first: 24 })).rejects.toMatchObject({
      status: 503,
    });
  });
});

describe("fetchProfileAssets", () => {
  it("reads every category when none is given, and maps a listed wearable", async () => {
    json.mockResolvedValue({ data: [token({}, { price: MANA(12), status: "open" })], total: 3431 });

    const page = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(json).toHaveBeenCalledWith("/marketplace-api/v1/nfts", {
      query: { owner: CREATOR.toLowerCase(), category: undefined, first: 24, skip: 0, sortBy: "newest" },
    });
    expect(page.total).toBe(3431);
    expect(page.items[0]).toEqual({
      id: `${CONTRACT}-54`,
      contractAddress: CONTRACT.toLowerCase(),
      tokenId: "54",
      category: "wearable",
      name: "Power Up!",
      image: "https://peer.example/image",
      rarity: "epic",
      network: "polygon",
      price: "12",
      traits: { category: "top_head", bodyShapes: ["BaseFemale"], isSmart: true },
    });
  });

  it("gives a NAME its label without the suffix, and no rarity or traits", async () => {
    json.mockResolvedValue({
      data: [token({ category: "ens", name: "Brai.dcl.eth", network: "ETHEREUM", data: { ens: { subdomain: "brai" } } })],
      total: 1,
    });

    const { items } = await fetchProfileAssets(CREATOR, { category: "ens", first: 24 });

    expect(items[0]).toMatchObject({ category: "ens", name: "Brai", rarity: undefined, network: "ethereum", price: undefined });
    expect(items[0]?.traits).toEqual({ category: undefined, bodyShapes: [], isSmart: false });
  });

  it.each(["javascript:alert(1)", "http://peer.example/image"])("drops the image %s, keeping only https", async (image) => {
    json.mockResolvedValue({ data: [token({ image })], total: 1 });

    const { items } = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(items[0]?.image).toBeUndefined();
  });

  it("names a parcel without a name by its coordinates, and keeps a row whose name is null", async () => {
    json.mockResolvedValue({
      data: [
        token({ category: "parcel", name: "", data: { parcel: { x: "162", y: "-133", description: null, estate: null } } }),
        token({ id: "e-1", category: "estate", name: null, data: { estate: { size: 2, description: null, parcels: [] } } }),
        token({ id: "n-1", category: "ens", name: ".dcl.eth", data: { ens: { subdomain: "" } } }),
        token({ id: "w-1", name: null }),
      ],
      total: 4,
    });

    const { items } = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(items.map((row) => row.name)).toEqual(["Parcel 162,-133", "Estate", "Untitled", "Untitled"]);
  });

  it("keeps a parcel's own name when it has one", async () => {
    json.mockResolvedValue({ data: [token({ category: "parcel", name: "Genesis Plaza", data: { parcel: { x: "0", y: "0" } } })], total: 1 });

    const { items } = await fetchProfileAssets(CREATOR, { category: "parcel", first: 24 });

    expect(items[0]?.name).toBe("Genesis Plaza");
  });

  it("fails on a response without the paged shape", async () => {
    json.mockResolvedValue({ data: { elements: [] } });

    await expect(fetchProfileAssets(CREATOR, { first: 24 })).rejects.toBeInstanceOf(CatalystError);
  });
});

describe("manaPrice", () => {
  it.each([
    [MANA(1), "1"],
    ["1234500000000000000000", "1,234.5"],
  ])("prints %s wei as %s", (wei, shown) => {
    expect(manaPrice(wei)).toBe(shown);
  });

  it.each([null, undefined, "", "0", "-5", "1.5", "abc"])("gives %j no price", (wei) => {
    expect(manaPrice(wei)).toBeUndefined();
  });
});
