import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../client", async (original) => ({
  ...(await original<typeof import("../client")>()),
  getJSON: vi.fn(),
}));
vi.mock("../warn", () => ({ warnInvalid: vi.fn() }));

import { CatalystError, getJSON } from "../client";
import { warnInvalid } from "../warn";
import { fetchProfileAssets, fetchProfileCreations, manaPrice } from "./profile-collectibles";

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
    price: MANA(369),
    minListingPrice: null,
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
afterEach(() => vi.resetAllMocks());

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
          price: "369",
          traits: { category: "upper_body", bodyShapes: ["BaseMale", "BaseFemale"], isSmart: false },
        },
      ],
    });
  });

  it("prices a sold-out item at its cheapest listing", async () => {
    json.mockResolvedValue({ data: [item({ price: "0", minListingPrice: MANA(1500) })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.price).toBe("1,500");
  });

  it("leaves an item with no sale and no listing without a price", async () => {
    json.mockResolvedValue({ data: [item({ price: "0", minListingPrice: null })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.price).toBeUndefined();
  });

  it("keeps an item whose optional facts are missing, without inventing them", async () => {
    json.mockResolvedValue({
      data: [item({ thumbnail: "", rarity: "legendaryish", network: "ETHEREUM", data: null })],
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

  it("drops an image that is not an http(s) URL", async () => {
    json.mockResolvedValue({ data: [token({ image: "javascript:alert(1)" })], total: 1 });

    const { items } = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(items[0]?.image).toBeUndefined();
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
