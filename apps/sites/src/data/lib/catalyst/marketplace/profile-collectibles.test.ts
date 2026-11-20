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
import {
  CREATIONS_QUOTES_TIMEOUT_MS,
  fetchProfileAssets,
  fetchProfileCreations,
  fetchWornCollectibles,
  manaPrice,
} from "./profile-collectibles";

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

  it("prices a sold-out primary item by its cheapest resale, never its mint price", async () => {
    json.mockResolvedValue({ data: [item({ available: 0, minListingPrice: MANA(80) })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "mana", mana: "80" });
  });

  it("gives a sold-out primary item with no resale no price, though the catalogue still marks it on sale", async () => {
    json.mockResolvedValue({ data: [item({ available: 0 })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items[0]?.payment).toEqual({ kind: "none" });
  });

  it.each([null, undefined])("keeps a row whose supply is %s and prices it as on sale", async (available) => {
    json.mockResolvedValue({ data: [item({ available })], total: 1 });

    const { items } = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

    expect(items).toHaveLength(1);
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

  it("names a parcel the catalogue calls by its category by its coordinates", async () => {
    // What the catalyst sends for an unnamed token: its capitalized category.
    json.mockResolvedValue({
      data: [
        token({ category: "parcel", name: "Parcel", data: { parcel: { x: "162", y: "-133", description: null, estate: null } } }),
        token({ id: "e-1", category: "estate", name: "Estate", data: { estate: { size: 2, description: null, parcels: [] } } }),
        token({ id: "p-2", category: "parcel", name: "Parcel", data: { parcel: { x: null, y: null } } }),
      ],
      total: 3,
    });

    const { items } = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(items.map((row) => row.name)).toEqual(["Parcel 162,-133", "Estate", "Untitled"]);
  });

  it("keeps real names that match their category, and names a NAME by its subdomain without one", async () => {
    json.mockResolvedValue({
      data: [
        token({ id: "n-1", category: "ens", name: "ens.dcl.eth", data: { ens: { subdomain: "ens" } } }),
        token({ id: "n-2", category: "ens", name: "", data: { ens: { subdomain: "brai" } } }),
        token({ id: "w-1", category: "wearable", name: "Wearable" }),
      ],
      total: 3,
    });

    const { items } = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(items.map((row) => row.name)).toEqual(["ens", "brai", "Wearable"]);
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

  it("treats the catalyst's \"Ens\" placeholder as no name, and empty coordinates as none", async () => {
    json.mockResolvedValue({
      data: [
        token({ id: "n-1", category: "ens", name: "Ens", data: { ens: { subdomain: "brai" } } }),
        token({ id: "n-2", category: "ens", name: "Ens", data: { ens: { subdomain: "" } } }),
        token({ id: "p-1", category: "parcel", name: "Parcel", data: { parcel: { x: "", y: "" } } }),
        token({ id: "p-2", category: "parcel", name: "Parcel", data: { parcel: { x: "7", y: " " } } }),
        token({ id: "p-3", category: "parcel", name: "Parcel", data: { parcel: { x: 0, y: -3 } } }),
      ],
      total: 5,
    });

    const { items } = await fetchProfileAssets(CREATOR, { first: 24 });

    expect(items.map((row) => row.name)).toEqual(["brai", "Untitled", "Untitled", "Untitled", "Parcel 0,-3"]);
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

describe("fetchWornCollectibles", () => {
  const OTHER = "0x1111111111111111111111111111111111111111";
  const matic = (contract: string, id: string) => `urn:decentraland:matic:collections-v2:${contract.toLowerCase()}:${id}`;
  const ETH_URN = "urn:decentraland:ethereum:collections-v1:halloween_2019:funny_skull_mask";

  // A `collections-v1` row has a contract address and a number where its URN has names.
  const mask = (over: Record<string, unknown> = {}) =>
    item({ id: `${OTHER}-34`, contractAddress: OTHER, itemId: "34", name: "Mask", network: "ETHEREUM", chainId: 1, urn: ETH_URN, ...over });

  it("reads the worn items by URN, one read per network, and keeps the order worn", async () => {
    vi.mocked(tryQuoteCreditItems).mockResolvedValue([null, null, null]);
    vi.mocked(getJSON).mockImplementation(async (path: string) =>
      path.includes("network=MATIC")
        ? // The catalogue answers in its own order.
          {
            data: [
              item({ id: `${OTHER}-7`, contractAddress: OTHER, itemId: "7", name: "Second", urn: matic(OTHER, "7") }),
              item({ name: "First", urn: matic(CONTRACT, "0") }),
            ],
            total: 2,
          }
        : { data: [mask()], total: 1 },
    );
    const signal = new AbortController().signal;

    const cards = await fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN, matic(OTHER, "7")], { signal });

    expect(cards.map((card) => card.name)).toEqual(["First", "Mask", "Second"]);
    const paths = vi.mocked(getJSON).mock.calls.map(([path]) => new URL(path, "http://node"));
    expect(paths.map((url) => url.pathname)).toEqual(["/marketplace-api/v2/catalog", "/marketplace-api/v2/catalog"]);
    const polygon = paths.find((url) => url.searchParams.get("network") === "MATIC");
    expect(polygon?.searchParams.getAll("urn")).toEqual([matic(CONTRACT, "0"), matic(OTHER, "7")]);
    expect(polygon?.searchParams.get("first")).toBe("2");
    const ethereum = paths.find((url) => url.searchParams.get("network") === "ETHEREUM");
    expect(ethereum?.searchParams.getAll("urn")).toEqual([ETH_URN]);
    expect(vi.mocked(getJSON).mock.calls[0]?.[1]).toEqual({ signal });
  });

  it("keeps the order worn for Ethereum items worn under the `mainnet` spelling of their URN", async () => {
    vi.mocked(tryQuoteCreditItems).mockResolvedValue([null, null]);
    const hat = "urn:decentraland:mainnet:collections-v1:Atari_Launch:atari_hat";
    vi.mocked(getJSON).mockResolvedValue({
      // The catalogue answers `ethereum`, in lowercase, and in its own order.
      data: [mask(), mask({ id: `${OTHER}-9`, itemId: "9", name: "Hat", urn: "urn:decentraland:ethereum:collections-v1:atari_launch:atari_hat" })],
      total: 2,
    });

    const cards = await fetchWornCollectibles([hat, ETH_URN]);

    expect(cards.map((card) => card.name)).toEqual(["Hat", "Mask"]);
    expect(new URL(vi.mocked(getJSON).mock.calls[0]![0], "http://node").searchParams.getAll("urn")).toEqual([hat, ETH_URN]);
  });

  it.each([
    ["Ethereum", "network=ETHEREUM", ["First"]],
    ["Polygon", "network=MATIC", ["Mask"]],
  ])("returns the other network's cards when the %s read fails", async (_network, failing, names) => {
    vi.mocked(tryQuoteCreditItems).mockResolvedValue([null]);
    vi.mocked(getJSON).mockImplementation(async (path: string) => {
      if (path.includes(failing)) throw new CatalystError("Catalyst returned 502", "/marketplace-api/v2/catalog", 502);
      return path.includes("network=MATIC")
        ? { data: [item({ name: "First", urn: matic(CONTRACT, "0") })], total: 1 }
        : { data: [mask()], total: 1 };
    });

    const cards = await fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN]);

    expect(cards.map((card) => card.name)).toEqual(names);
  });

  it("fails when both networks' reads fail", async () => {
    vi.mocked(getJSON).mockRejectedValue(new CatalystError("Catalyst returned 502", "/marketplace-api/v2/catalog", 502));

    await expect(fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN])).rejects.toBeInstanceOf(CatalystError);
    expect(getJSON).toHaveBeenCalledTimes(2);
  });

  it("stops, with no partial answer, when the visitor leaves while one network's read runs", async () => {
    const controller = new AbortController();
    vi.mocked(getJSON).mockImplementation(async (path: string) => {
      if (path.includes("network=MATIC")) return { data: [item({ urn: matic(CONTRACT, "0") })], total: 1 };
      controller.abort();
      throw controller.signal.reason;
    });

    await expect(fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN], { signal: controller.signal })).rejects.toBe(
      controller.signal.reason,
    );
    expect(tryQuoteCreditItems).not.toHaveBeenCalled();
  });

  it("asks nothing of a network with no worn item, and prices a card as the Shop does", async () => {
    vi.mocked(tryQuoteCreditItems).mockResolvedValue(["12"]);
    vi.mocked(getJSON).mockResolvedValue({ data: [item()], total: 1 });

    const cards = await fetchWornCollectibles([matic(CONTRACT, "0")]);

    expect(getJSON).toHaveBeenCalledTimes(1);
    expect(cards[0]?.payment).toEqual({ kind: "credits", credits: "12" });
  });

  it("leaves out a worn item the catalogue does not know", async () => {
    vi.mocked(getJSON).mockResolvedValue({ data: [], total: 0 });

    await expect(fetchWornCollectibles([matic(CONTRACT, "0")])).resolves.toEqual([]);
    expect(tryQuoteCreditItems).not.toHaveBeenCalled();
  });

  it("passes a service failure through", async () => {
    vi.mocked(getJSON).mockRejectedValue(new CatalystError("Catalyst returned 502", "/marketplace-api/v2/catalog", 502));

    await expect(fetchWornCollectibles([matic(CONTRACT, "0")])).rejects.toBeInstanceOf(CatalystError);
  });
});
