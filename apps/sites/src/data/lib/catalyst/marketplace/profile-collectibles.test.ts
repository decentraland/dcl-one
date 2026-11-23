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
  fetchProfileAssetCount,
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
afterEach(() => {
  vi.useRealTimers();
  vi.resetAllMocks();
});

type CreationsPage = Awaited<ReturnType<typeof fetchProfileCreations>>;
type AssetsPage = Awaited<ReturnType<typeof fetchProfileAssets>>;

describe("fetchProfileCreations", () => {
  const read = () => fetchProfileCreations(CREATOR, { category: "wearable", first: 24 });

  beforeEach(() => {
    quotes.mockImplementation(async (refs) => refs.map(() => "12"));
  });

  describe("when the page brings a primary Polygon wearable", () => {
    let page: CreationsPage;

    beforeEach(async () => {
      json.mockResolvedValueOnce({ data: [item()], total: 12 });
      page = await fetchProfileCreations(CREATOR, { category: "wearable", first: 24, skip: 24 });
    });

    it("should read one category of the lowercased creator's items, newest first", () => {
      expect(json).toHaveBeenCalledWith("/marketplace-api/v2/catalog", {
        query: { creator: CREATOR.toLowerCase(), category: "wearable", first: 24, skip: 24, sortBy: "newest" },
      });
    });

    it("should map each card", () => {
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
    });

    it("should quote the item's credit price", () => {
      expect(quotes).toHaveBeenCalledWith([{ itemId: "0", collection: CONTRACT.toLowerCase() }], expect.anything());
    });
  });

  describe("when the page brings no item", () => {
    beforeEach(async () => {
      json.mockResolvedValueOnce({ data: [], total: 0 });
      await read();
    });

    it("should ask for no quote", () => {
      expect(quotes).not.toHaveBeenCalled();
    });
  });

  describe("when the quote fails", () => {
    beforeEach(() => {
      quotes.mockResolvedValueOnce([null]);
      json.mockResolvedValueOnce({ data: [item()], total: 1 });
    });

    it("should say the credit price is unavailable, never falling back to MANA", async () => {
      const { items } = await read();
      expect(items[0]?.payment).toEqual({ kind: "creditsUnavailable" });
    });
  });

  describe("when the quote outlasts its deadline", () => {
    let pending: Promise<CreationsPage>;

    beforeEach(async () => {
      vi.useFakeTimers();
      quotes.mockImplementation(() => new Promise(() => {}));
      json.mockResolvedValueOnce({ data: [item()], total: 1 });
      pending = read();
      await vi.advanceTimersByTimeAsync(CREATIONS_QUOTES_TIMEOUT_MS + 1);
    });

    it("should treat the quote as unavailable", async () => {
      expect((await pending).items[0]?.payment).toEqual({ kind: "creditsUnavailable" });
    });
  });

  describe("when the visitor leaves while the quote runs", () => {
    let leave: AbortController;

    beforeEach(() => {
      leave = new AbortController();
      quotes.mockImplementation(() => new Promise(() => {}));
      json.mockResolvedValueOnce({ data: [item()], total: 1 });
    });

    it("should stop with the leave reason", async () => {
      const pending = fetchProfileCreations(CREATOR, { category: "wearable", first: 24 }, { signal: leave.signal });
      leave.abort(new Error("left"));
      await expect(pending).rejects.toThrow("left");
    });
  });

  describe("when the item mints on Ethereum", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({ data: [item({ network: "ETHEREUM", chainId: 1 })], total: 1 });
    });

    it("should price it in MANA, which credits cannot buy", async () => {
      const { items } = await read();
      expect(items[0]?.payment).toEqual({ kind: "mana", mana: "369" });
    });
  });

  describe("when the primary price is zero", () => {
    describe("and the item has a listing", () => {
      beforeEach(() => {
        json.mockResolvedValueOnce({ data: [item({ price: "0", minListingPrice: MANA(1500) })], total: 1 });
      });

      it("should fall back to the cheapest listing", async () => {
        const { items } = await read();
        expect(items[0]?.payment).toEqual({ kind: "mana", mana: "1,500" });
      });
    });

    describe("and the item has no listing", () => {
      beforeEach(() => {
        json.mockResolvedValueOnce({ data: [item({ price: "0", minListingPrice: null })], total: 1 });
      });

      it("should leave the item without a price", async () => {
        const { items } = await read();
        expect(items[0]?.payment).toEqual({ kind: "none" });
      });
    });
  });

  describe("when the primary item is sold out", () => {
    describe("and it has a resale", () => {
      beforeEach(() => {
        json.mockResolvedValueOnce({ data: [item({ available: 0, minListingPrice: MANA(80) })], total: 1 });
      });

      it("should price it by its cheapest resale, never its mint price", async () => {
        const { items } = await read();
        expect(items[0]?.payment).toEqual({ kind: "mana", mana: "80" });
      });
    });

    describe("and it has no resale", () => {
      beforeEach(() => {
        json.mockResolvedValueOnce({ data: [item({ available: 0 })], total: 1 });
      });

      it("should give it no price, though the catalogue still marks it on sale", async () => {
        const { items } = await read();
        expect(items[0]?.payment).toEqual({ kind: "none" });
      });
    });
  });

  describe.each([null, undefined])("when the row's supply is %s", (available) => {
    let items: CreationsPage["items"];

    beforeEach(async () => {
      json.mockResolvedValueOnce({ data: [item({ available })], total: 1 });
      ({ items } = await read());
    });

    it("should keep the row", () => {
      expect(items).toHaveLength(1);
    });

    it("should price it as on sale", () => {
      expect(items[0]?.payment).toEqual({ kind: "credits", credits: "12" });
    });
  });

  describe("when the item is delisted with no listing", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({ data: [item({ isOnSale: false })], total: 1 });
    });

    it("should give it no price, though the catalogue keeps its old one", async () => {
      const { items } = await read();
      expect(items[0]?.payment).toEqual({ kind: "none" });
    });
  });

  describe("when the item's name is blank", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({ data: [item({ name: "  " })], total: 1 });
    });

    it("should name it Untitled", async () => {
      const { items } = await read();
      expect(items[0]?.name).toBe("Untitled");
    });
  });

  describe("when the item's optional facts are missing", () => {
    let items: CreationsPage["items"];

    beforeEach(async () => {
      json.mockResolvedValueOnce({
        data: [item({ thumbnail: "", rarity: "legendaryish", network: "ETHEREUM", chainId: 1, data: null })],
        total: 1,
      });
      ({ items } = await fetchProfileCreations(CREATOR, { category: "emote", first: 24 }));
    });

    it("should keep the item without inventing its thumbnail or rarity", () => {
      expect(items[0]).toMatchObject({ thumbnail: undefined, rarity: undefined, network: "ethereum" });
    });

    it("should give it empty traits", () => {
      expect(items[0]?.traits).toEqual({ category: undefined, bodyShapes: [], isSmart: false });
    });
  });

  describe("when a row has no identity", () => {
    let page: CreationsPage;

    beforeEach(async () => {
      json.mockResolvedValueOnce({ data: [item({ id: undefined }), item({ id: "b", itemId: "1" })], total: 2 });
      page = await read();
    });

    it("should leave it out", () => {
      expect(page.items.map((row) => row.id)).toEqual(["b"]);
    });

    it("should keep the catalogue's total", () => {
      expect(page.total).toBe(2);
    });

    it("should still count it as received", () => {
      expect(page.received).toBe(2);
    });

    it("should report it", () => {
      expect(warnInvalid).toHaveBeenCalledWith("ProfileCreation", expect.anything());
    });
  });

  describe("when the response does not have the paged shape", () => {
    beforeEach(() => {
      json.mockResolvedValue({ ok: false, error: "404 path not found" });
    });

    it("should fail instead of showing an empty list", async () => {
      await expect(read()).rejects.toBeInstanceOf(CatalystError);
    });

    it("should say the response is not paged", async () => {
      await expect(read()).rejects.toThrow("the response did not have the paged shape");
    });
  });

  describe("when the service fails", () => {
    beforeEach(() => {
      json.mockRejectedValueOnce(new CatalystError("Catalyst returned 503", "/marketplace-api/v2/catalog", 503));
    });

    it("should pass the failure through", async () => {
      await expect(read()).rejects.toMatchObject({
        status: 503,
        message: "Catalyst returned 503",
      });
    });
  });
});

describe("fetchProfileAssets", () => {
  const read = () => fetchProfileAssets(CREATOR, { first: 24 });

  describe("when no category is given", () => {
    let page: AssetsPage;

    beforeEach(async () => {
      json.mockResolvedValueOnce({ data: [token({}, { price: MANA(12), status: "open" })], total: 3431 });
      page = await read();
    });

    it("should read every category", () => {
      expect(json).toHaveBeenCalledWith("/marketplace-api/v1/nfts", {
        query: { owner: CREATOR.toLowerCase(), category: undefined, first: 24, skip: 0, sortBy: "newest" },
      });
    });

    it("should answer the inventory's total", () => {
      expect(page.total).toBe(3431);
    });

    it("should map a listed wearable", () => {
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
  });

  describe("when the token is a NAME", () => {
    let items: AssetsPage["items"];

    beforeEach(async () => {
      json.mockResolvedValueOnce({
        data: [token({ category: "ens", name: "Brai.dcl.eth", network: "ETHEREUM", data: { ens: { subdomain: "brai" } } })],
        total: 1,
      });
      ({ items } = await fetchProfileAssets(CREATOR, { category: "ens", first: 24 }));
    });

    it("should give it its label without the suffix, and no rarity", () => {
      expect(items[0]).toMatchObject({ category: "ens", name: "Brai", rarity: undefined, network: "ethereum", price: undefined });
    });

    it("should give it no traits", () => {
      expect(items[0]?.traits).toEqual({ category: undefined, bodyShapes: [], isSmart: false });
    });
  });

  describe.each(["javascript:alert(1)", "http://peer.example/image"])("when the image is %s", (image) => {
    beforeEach(() => {
      json.mockResolvedValueOnce({ data: [token({ image })], total: 1 });
    });

    it("should drop it, keeping only https", async () => {
      const { items } = await read();
      expect(items[0]?.image).toBeUndefined();
    });
  });

  describe("when the catalogue calls a parcel by its category", () => {
    beforeEach(() => {
      // What the catalyst sends for an unnamed token: its capitalized category.
      json.mockResolvedValueOnce({
        data: [
          token({ category: "parcel", name: "Parcel", data: { parcel: { x: "162", y: "-133", description: null, estate: null } } }),
          token({ id: "e-1", category: "estate", name: "Estate", data: { estate: { size: 2, description: null, parcels: [] } } }),
          token({ id: "p-2", category: "parcel", name: "Parcel", data: { parcel: { x: null, y: null } } }),
        ],
        total: 3,
      });
    });

    it("should name the parcel by its coordinates", async () => {
      const { items } = await read();
      expect(items.map((row) => row.name)).toEqual(["Parcel 162,-133", "Estate", "Untitled"]);
    });
  });

  describe("when real names match their category", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({
        data: [
          token({ id: "n-1", category: "ens", name: "ens.dcl.eth", data: { ens: { subdomain: "ens" } } }),
          token({ id: "n-2", category: "ens", name: "", data: { ens: { subdomain: "brai" } } }),
          token({ id: "w-1", category: "wearable", name: "Wearable" }),
        ],
        total: 3,
      });
    });

    it("should keep them, and name a NAME without one by its subdomain", async () => {
      const { items } = await read();
      expect(items.map((row) => row.name)).toEqual(["ens", "brai", "Wearable"]);
    });
  });

  describe("when names are empty or null", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({
        data: [
          token({ category: "parcel", name: "", data: { parcel: { x: "162", y: "-133", description: null, estate: null } } }),
          token({ id: "e-1", category: "estate", name: null, data: { estate: { size: 2, description: null, parcels: [] } } }),
          token({ id: "n-1", category: "ens", name: ".dcl.eth", data: { ens: { subdomain: "" } } }),
          token({ id: "w-1", name: null }),
        ],
        total: 4,
      });
    });

    it("should name a parcel by its coordinates and keep a row whose name is null", async () => {
      const { items } = await read();
      expect(items.map((row) => row.name)).toEqual(["Parcel 162,-133", "Estate", "Untitled", "Untitled"]);
    });
  });

  describe("when the catalyst sends the \"Ens\" placeholder or empty coordinates", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({
        data: [
          token({ id: "n-1", category: "ens", name: "Ens", data: { ens: { subdomain: "brai" } } }),
          token({ id: "n-2", category: "ens", name: "Ens", data: { ens: { subdomain: "" } } }),
          token({ id: "p-1", category: "parcel", name: "Parcel", data: { parcel: { x: "", y: "" } } }),
          token({ id: "p-2", category: "parcel", name: "Parcel", data: { parcel: { x: "7", y: " " } } }),
          token({ id: "p-3", category: "parcel", name: "Parcel", data: { parcel: { x: 0, y: -3 } } }),
        ],
        total: 5,
      });
    });

    it("should treat them as no name and no coordinates", async () => {
      const { items } = await read();
      expect(items.map((row) => row.name)).toEqual(["brai", "Untitled", "Untitled", "Untitled", "Parcel 0,-3"]);
    });
  });

  describe("when a parcel has its own name", () => {
    beforeEach(() => {
      json.mockResolvedValueOnce({ data: [token({ category: "parcel", name: "Genesis Plaza", data: { parcel: { x: "0", y: "0" } } })], total: 1 });
    });

    it("should keep it", async () => {
      const { items } = await fetchProfileAssets(CREATOR, { category: "parcel", first: 24 });
      expect(items[0]?.name).toBe("Genesis Plaza");
    });
  });

  describe("when the response does not have the paged shape", () => {
    beforeEach(() => {
      json.mockResolvedValue({ data: { elements: [] } });
    });

    it("should fail", async () => {
      await expect(read()).rejects.toBeInstanceOf(CatalystError);
    });

    it("should say the response is not paged", async () => {
      await expect(read()).rejects.toThrow("the response did not have the paged shape");
    });
  });
});

describe("manaPrice", () => {
  describe("when the value is a positive whole number of wei", () => {
    it.each([
      [MANA(1), "1"],
      ["1234500000000000000000", "1,234.5"],
    ])("should print %s wei as %s", (wei, shown) => {
      expect(manaPrice(wei)).toBe(shown);
    });
  });

  describe("when the value is missing, zero, negative or not a whole number", () => {
    it.each([null, undefined, "", "0", "-5", "1.5", "abc"])("should give %j no price", (wei) => {
      expect(manaPrice(wei)).toBeUndefined();
    });
  });
});

describe("fetchWornCollectibles", () => {
  const OTHER = "0x1111111111111111111111111111111111111111";
  const matic = (contract: string, id: string) => `urn:decentraland:matic:collections-v2:${contract.toLowerCase()}:${id}`;
  const ETH_URN = "urn:decentraland:ethereum:collections-v1:halloween_2019:funny_skull_mask";

  // A `collections-v1` row has a contract address and a number where its URN has names.
  const mask = (over: Record<string, unknown> = {}) =>
    item({ id: `${OTHER}-34`, contractAddress: OTHER, itemId: "34", name: "Mask", network: "ETHEREUM", chainId: 1, urn: ETH_URN, ...over });

  const catalogPaths = () => vi.mocked(getJSON).mock.calls.map(([path]) => new URL(path, "http://node"));

  beforeEach(() => {
    quotes.mockImplementation(async (refs) => refs.map(() => "12"));
  });

  describe("when the worn items are on both networks", () => {
    let signal: AbortSignal;
    let cards: Awaited<ReturnType<typeof fetchWornCollectibles>>;

    beforeEach(async () => {
      vi.mocked(tryQuoteCreditItems).mockResolvedValueOnce([null, null, null]);
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
      signal = new AbortController().signal;
      cards = await fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN, matic(OTHER, "7")], { signal });
    });

    it("should keep the order worn", () => {
      expect(cards.map((card) => card.name)).toEqual(["First", "Mask", "Second"]);
    });

    it("should make one catalogue read per network", () => {
      expect(catalogPaths().map((url) => url.pathname)).toEqual(["/marketplace-api/v2/catalog", "/marketplace-api/v2/catalog"]);
    });

    it("should read the Polygon items by URN", () => {
      const polygon = catalogPaths().find((url) => url.searchParams.get("network") === "MATIC");
      expect(polygon?.searchParams.getAll("urn")).toEqual([matic(CONTRACT, "0"), matic(OTHER, "7")]);
      expect(polygon?.searchParams.get("first")).toBe("2");
    });

    it("should read the Ethereum items by URN", () => {
      const ethereum = catalogPaths().find((url) => url.searchParams.get("network") === "ETHEREUM");
      expect(ethereum?.searchParams.getAll("urn")).toEqual([ETH_URN]);
    });

    it("should pass the caller's signal", () => {
      expect(vi.mocked(getJSON).mock.calls[0]?.[1]).toEqual({ signal });
    });
  });

  describe("when Ethereum items are worn under the `mainnet` spelling of their URN", () => {
    let hat: string;
    let cards: Awaited<ReturnType<typeof fetchWornCollectibles>>;

    beforeEach(async () => {
      hat = "urn:decentraland:mainnet:collections-v1:Atari_Launch:atari_hat";
      vi.mocked(tryQuoteCreditItems).mockResolvedValueOnce([null, null]);
      vi.mocked(getJSON).mockResolvedValueOnce({
        // The catalogue answers `ethereum`, in lowercase, and in its own order.
        data: [mask(), mask({ id: `${OTHER}-9`, itemId: "9", name: "Hat", urn: "urn:decentraland:ethereum:collections-v1:atari_launch:atari_hat" })],
        total: 2,
      });
      cards = await fetchWornCollectibles([hat, ETH_URN]);
    });

    it("should keep the order worn", () => {
      expect(cards.map((card) => card.name)).toEqual(["Hat", "Mask"]);
    });

    it("should ask for the URNs as worn", () => {
      expect(new URL(vi.mocked(getJSON).mock.calls[0]![0], "http://node").searchParams.getAll("urn")).toEqual([hat, ETH_URN]);
    });
  });

  describe.each([
    ["Ethereum", "network=ETHEREUM", ["First"]],
    ["Polygon", "network=MATIC", ["Mask"]],
  ])("when the %s read fails", (_network, failing, names) => {
    beforeEach(() => {
      vi.mocked(tryQuoteCreditItems).mockResolvedValueOnce([null]);
      vi.mocked(getJSON).mockImplementation(async (path: string) => {
        if (path.includes(failing)) throw new CatalystError("Catalyst returned 502", "/marketplace-api/v2/catalog", 502);
        return path.includes("network=MATIC")
          ? { data: [item({ name: "First", urn: matic(CONTRACT, "0") })], total: 1 }
          : { data: [mask()], total: 1 };
      });
    });

    it("should return the other network's cards", async () => {
      const cards = await fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN]);
      expect(cards.map((card) => card.name)).toEqual(names);
    });
  });

  describe("when both networks' reads fail", () => {
    let result: Promise<unknown>;

    beforeEach(() => {
      vi.mocked(getJSON).mockRejectedValue(new CatalystError("Catalyst returned 502", "/marketplace-api/v2/catalog", 502));
      result = fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN]);
    });

    it("should fail", async () => {
      await expect(result).rejects.toBeInstanceOf(CatalystError);
    });

    it("should say the catalogue failed", async () => {
      await expect(result).rejects.toThrow("Catalyst returned 502");
    });

    it("should have tried both networks", async () => {
      await result.catch(() => undefined);
      expect(getJSON).toHaveBeenCalledTimes(2);
    });
  });

  describe("when the visitor leaves while one network's read runs", () => {
    let controller: AbortController;
    let result: Promise<unknown>;

    beforeEach(() => {
      controller = new AbortController();
      vi.mocked(getJSON).mockImplementation(async (path: string) => {
        if (path.includes("network=MATIC")) return { data: [item({ urn: matic(CONTRACT, "0") })], total: 1 };
        controller.abort();
        throw controller.signal.reason;
      });
      result = fetchWornCollectibles([matic(CONTRACT, "0"), ETH_URN], { signal: controller.signal });
    });

    it("should stop with the leave reason", async () => {
      await expect(result).rejects.toBe(controller.signal.reason);
    });

    it("should give no partial answer", async () => {
      await result.catch(() => undefined);
      expect(tryQuoteCreditItems).not.toHaveBeenCalled();
    });
  });

  describe("when only one network has a worn item", () => {
    let cards: Awaited<ReturnType<typeof fetchWornCollectibles>>;

    beforeEach(async () => {
      vi.mocked(tryQuoteCreditItems).mockResolvedValueOnce(["12"]);
      vi.mocked(getJSON).mockResolvedValueOnce({ data: [item()], total: 1 });
      cards = await fetchWornCollectibles([matic(CONTRACT, "0")]);
    });

    it("should ask nothing of the other network", () => {
      expect(getJSON).toHaveBeenCalledTimes(1);
    });

    it("should price a card as the Shop does", () => {
      expect(cards[0]?.payment).toEqual({ kind: "credits", credits: "12" });
    });
  });

  describe("when the catalogue does not know a worn item", () => {
    let result: Promise<unknown>;

    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce({ data: [], total: 0 });
      result = fetchWornCollectibles([matic(CONTRACT, "0")]);
    });

    it("should leave it out", async () => {
      await expect(result).resolves.toEqual([]);
    });

    it("should ask for no quote", async () => {
      await result;
      expect(tryQuoteCreditItems).not.toHaveBeenCalled();
    });
  });

  describe("when the service fails", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockRejectedValueOnce(new CatalystError("Catalyst returned 502", "/marketplace-api/v2/catalog", 502));
    });

    it("should pass the failure through", async () => {
      await expect(fetchWornCollectibles([matic(CONTRACT, "0")])).rejects.toBeInstanceOf(CatalystError);
    });

    it("should keep the failure's message", async () => {
      await expect(fetchWornCollectibles([matic(CONTRACT, "0")])).rejects.toThrow("Catalyst returned 502");
    });
  });
});

describe("when checking whether the owner has assets in a category", () => {
  let result: Promise<number>;

  beforeEach(() => {
    json.mockResolvedValueOnce({ data: [token({})], total: 30 });
    result = fetchProfileAssetCount(CREATOR, "ens");
  });

  it("should use the inventory total even though only one result is requested", async () => {
    await expect(result).resolves.toBe(30);
  });

  it("should request one result of the category", async () => {
    await result;
    expect(json).toHaveBeenCalledWith("/marketplace-api/v1/nfts", {
      query: { owner: CREATOR.toLowerCase(), category: "ens", first: 1, skip: 0, sortBy: "newest" },
    });
  });
});
