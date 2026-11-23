import { QueryClient } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { input } from "zod";

import type { CatalogItemSchema } from "../catalyst/generated-schemas/market";

import { reelWearables, reelWearablesQuery, wearableRef } from "./queries";

const CONTRACT = "0x1111111111111111111111111111111111111111";
const ID = `${CONTRACT}-0`;
const ASSET = `urn:decentraland:matic:collections-v2:${CONTRACT}:0`;
const TOKEN = `${ASSET}:603`;
const ETHEREUM = `urn:decentraland:ethereum:collections-v2:${CONTRACT}:0`;
const LEGACY = `urn:decentraland:mainnet:collections-v2:${CONTRACT}:0`;
const UNRESOLVED = { id: "unavailable", label: "Some collectible details are unavailable." };
const HAT = {
  id: ID,
  beneficiary: null,
  itemId: "0",
  name: "Hat",
  thumbnail: "https://reel.example/hat.png",
  url: "",
  urn: ASSET,
  category: "wearable",
  contractAddress: CONTRACT,
  rarity: "common",
  available: 0,
  isOnSale: false,
  creator: CONTRACT,
  data: {},
  network: "MATIC",
  chainId: 137,
  price: "0",
  createdAt: 0,
  updatedAt: 0,
  reviewedAt: 0,
  firstListedAt: null,
  soldAt: 0,
  maxListingPrice: null,
  minListingPrice: null,
  listings: null,
  owners: null,
  picks: null,
} satisfies input<typeof CatalogItemSchema>;

afterEach(() => vi.unstubAllGlobals());

describe("Reels wearable lookups", () => {
  describe("when a photo names an equipped NFT instance", () => {
    it("should look up its catalog item instead of the instance", () => {
      expect(wearableRef(TOKEN)).toEqual(wearableRef(ASSET));
      expect(wearableRef(TOKEN)?.id).toBe(ID);
    });
  });

  describe("when the photo uses the Ethereum chain name", () => {
    it("should resolve the same Ethereum catalog item as the historical alias", () => {
      expect(wearableRef(ETHEREUM)).toEqual(wearableRef(LEGACY));
      expect(wearableRef(ETHEREUM)?.key).toBe(`1:${ID}`);
    });
  });

  describe("when the catalogue has partial or inconsistent data", () => {
    it("should keep resolved items and a text fallback for unavailable items", async () => {
      const fetch = vi.fn(async (href: string) => {
        const url = new URL(href);
        if (url.searchParams.get("network") === "ETHEREUM") return new Response(null, { status: 503 });
        expect(url.searchParams.getAll("id")).toEqual([ID]);
        return Response.json({
          total: 2,
          data: [
            { ...HAT, network: "ETHEREUM", chainId: 1, name: "Wrong chain" },
            HAT,
          ],
        });
      });
      vi.stubGlobal("fetch", fetch);
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      const urns = [ASSET, TOKEN, ETHEREUM, LEGACY, "urn:decentraland:off-chain:base-avatars:tshirt"];
      const items = await client.fetchQuery(reelWearablesQuery(urns));
      expect(fetch).toHaveBeenCalledTimes(2);
      expect(reelWearables(urns, items)).toEqual([
        expect.objectContaining({ id: `137:${ID}`, label: "Hat", href: `/shop/item/${CONTRACT}/0` }),
        UNRESOLVED,
      ]);
    });
  });

  describe("when two of a photo's URNs are the same catalog item", () => {
    it("should list the item once, under the first URN it was worn as", () => {
      const hat = { id: `137:${ID}`, label: "Hat" };
      expect(reelWearables([TOKEN, ASSET], { [`137:${ID}`]: hat })).toEqual([{ ...hat, urn: TOKEN }]);
    });
  });

  describe("when no item can be looked up", () => {
    it("should ignore unsupported entries and report missing catalog items", () => {
      expect(reelWearablesQuery(["broken"]).enabled).toBe(false);
      expect(reelWearables(["broken", "", "urn:decentraland:off-chain:base-avatars:tshirt"], {})).toEqual([]);
      expect(reelWearables([ASSET], {})).toEqual([UNRESOLVED]);
    });
  });
});
