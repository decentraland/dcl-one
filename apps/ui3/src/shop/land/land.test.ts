import { describe, expect, it } from "vitest";

import { landTokenId } from "../../data/catalyst/landListing";
import { CHAIN } from "../model/chain";
import {
  DEFAULT_LAND_FILTERS,
  fetchLandNfts,
  fetchNftResult,
  indexOutage,
  landData,
  landFiltersFromSearch,
  landFiltersToSearch,
  landNftsQuery,
  landParcels,
  landSortsFor,
} from "./api";
import { landCategoryOf, landNetwork, LAND_NETWORKS } from "./contracts";
import { computeEstateFingerprint, isEstateListingAffectedByUpgrade, ESTATE_V2_FINGERPRINT_LISTING_CUTOFF } from "./estate";
import { distanceText, estateProximity, parcelProximity, proximityIndex } from "./proximity";
import { stubClient } from "./testkit";
import {
  coordsId,
  decodeLandTokenId,
  fetchTiles,
  isTileInSelection,
  MapNotReadyError,
  nearestTile,
  parseLegacyTile,
  selectionCenter,
  tilesByEstateId,
  type Tiles,
} from "./tiles";

function grid(rows: string[], originX = 0, originY = 0): Tiles {
  const kinds: Record<string, "road" | "plaza" | "district" | "owned" | "unowned"> = { R: "road", P: "plaza", D: "district", o: "owned", _: "unowned" };
  const tiles: Tiles = {};
  rows.forEach((row, r) => {
    [...row].forEach((ch, c) => {
      const x = originX + c;
      const y = originY - r;
      tiles[coordsId(x, y)] = { id: coordsId(x, y), x, y, kind: kinds[ch]!, top: false, left: false, topLeft: false, owner: null, name: null, estateId: null, price: null, rentalPricePerDay: null };
    });
  });
  return tiles;
}

describe("the atlas tiles (FETCH_TILES_REQUEST)", () => {
  it("parses the map service's legacy tiles: kinds, sale and rent prices, estates", () => {
    expect(parseLegacyTile("-3,4", { type: 10, x: -3, y: 4, top: 1, owner: "0xAB", estate_id: "12", price: 1500, rentalPricePerDay: "2000000000000000000" })).toEqual({
      id: "-3,4",
      x: -3,
      y: 4,
      kind: "owned",
      top: true,
      left: false,
      topLeft: false,
      owner: "0xab",
      name: null,
      estateId: "12",
      price: 1500,
      rentalPricePerDay: "2000000000000000000",
    });
    expect(parseLegacyTile("0,0", { type: 7 })?.kind).toBe("road");
    expect(parseLegacyTile("0,1", { type: 8 })?.kind).toBe("plaza");
    expect(parseLegacyTile("0,2", { type: 5, rentalPricePerDay: "0" })).toMatchObject({ kind: "district", rentalPricePerDay: null });
  });

  it("fetches /v1/tiles from the map service and keeps its Last-Modified", async () => {
    let url = "";
    const res = await fetchTiles({
      base: "https://map.test",
      fetchImpl: async (input) => {
        url = String(input);
        return new Response(JSON.stringify({ data: { "1,1": { type: 11, x: 1, y: 1 } } }), { headers: { "last-modified": "Wed, 23 Sep 2026 10:00:00 GMT" } });
      },
    });
    expect(url).toBe("https://map.test/v1/tiles");
    expect(Object.keys(res.tiles)).toEqual(["1,1"]);
    expect(res.lastModified?.toISOString()).toBe("2026-09-23T10:00:00.000Z");
  });

  it("reports a grid that is not mirrored yet as an outage, not an empty city", async () => {
    await expect(fetchTiles({ base: "https://map.test", fetchImpl: async () => new Response("Not ready", { status: 503 }) })).rejects.toBeInstanceOf(MapNotReadyError);
  });

  it("groups estate tiles, finds the nearest tile, and expands an indexed estate selection unless strict", () => {
    const tiles = grid(["ooo"]);
    tiles["0,0"]!.estateId = "7";
    tiles["1,0"]!.estateId = "7";
    expect(Object.keys(tilesByEstateId(tiles))).toEqual(["7"]);
    expect(nearestTile({ x: 5, y: 5 }, [{ x: 0, y: 0 }, { x: 4, y: 4 }])).toEqual({ x: 4, y: 4 });
    const selection = new Set(["0,0"]);
    expect(isTileInSelection(selection, tiles, 1, 0, true, false)).toBe(true);
    expect(isTileInSelection(selection, tiles, 1, 0, true, true)).toBe(false);
    expect(isTileInSelection(selection, tiles, 2, 0, true, false)).toBe(false);
  });

  it("decodes LAND token ids with negative coordinates and centres a selection", () => {
    expect(decodeLandTokenId(landTokenId(-150, 42))).toEqual({ x: -150, y: 42 });
    expect(decodeLandTokenId(landTokenId(3, -7))).toEqual({ x: 3, y: -7 });
    expect(selectionCenter([{ x: 0, y: 0 }, { x: 2, y: 0 }, { x: 1, y: 1 }])).toEqual({ x: 1, y: 1 });
  });
});

describe("proximity (FETCH_PROXIMITY_REQUEST, derived with the generator's metric)", () => {
  const tiles = grid(["Ro_oo", "__o_P", "o____"]);
  const index = proximityIndex(tiles);

  it("an adjacent road is 0; distance is Manhattan minus one", () => {
    expect(parcelProximity({ x: 1, y: 0 }, index)).toEqual({ road: 0, plaza: 3 });
    expect(parcelProximity({ x: 3, y: -1 }, index)).toEqual({ road: 3, plaza: 0 });
  });

  it("an estate takes its closest parcel per kind", () => {
    expect(estateProximity([{ x: 0, y: -2 }, { x: 3, y: 0 }], index)).toEqual({ road: 1, plaza: 1 });
  });

  it("ignores anything 10 or more parcels away", () => {
    const far = grid(["R" + "_".repeat(12) + "o"]);
    expect(parcelProximity({ x: 13, y: 0 }, proximityIndex(far))).toBeUndefined();
    expect(distanceText(0)).toBe("Adjacent");
    expect(distanceText(1)).toBe("1 parcel away");
    expect(distanceText(4)).toBe("4 parcels away");
  });
});

describe("contracts and estates", () => {
  it("knows the LAND registries per network", () => {
    expect(landNetwork(CHAIN.ETHEREUM).land).toBe("0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d");
    expect(landCategoryOf("0x959E104E1A4DB6317FA58F8295F586E1A978C297")?.category).toBe("estate");
    expect(landCategoryOf(LAND_NETWORKS[CHAIN.SEPOLIA]!.land)?.network.chainId).toBe(CHAIN.SEPOLIA);
    expect(landCategoryOf("0x0000000000000000000000000000000000000001")).toBeNull();
    expect(() => landNetwork(CHAIN.POLYGON)).toThrow(RangeError);
  });

  it("computes getFingerprintV2 off-chain and flags only pre-upgrade listings on large estates", () => {
    expect(computeEstateFingerprint("1", ["1", "2"])).toMatch(/^0x[0-9a-f]{64}$/);
    expect(computeEstateFingerprint("1", ["1", "2"])).not.toBe(computeEstateFingerprint("1", ["2", "1"]));
    expect(isEstateListingAffectedByUpgrade(19, ESTATE_V2_FINGERPRINT_LISTING_CUTOFF - 1)).toBe(true);
    expect(isEstateListingAffectedByUpgrade(19, (ESTATE_V2_FINGERPRINT_LISTING_CUTOFF + 1) * 1000)).toBe(false);
    expect(isEstateListingAffectedByUpgrade(18, 0)).toBe(false);
    expect(isEstateListingAffectedByUpgrade(40, undefined)).toBe(false);
  });
});

describe("the LAND browse query", () => {
  it("round-trips the URL and defaults to LAND on sale", () => {
    expect(landFiltersFromSearch("")).toEqual(DEFAULT_LAND_FILTERS);
    const f = landFiltersFromSearch("?onlyOnRent=true&section=estates&minEstateSize=2&adjacentToRoad=true&rentalDays=7&rentalDays=30&page=3");
    expect(f).toMatchObject({ status: "rent", category: "estate", minEstateSize: "2", adjacentToRoad: true, rentalDays: [7, 30], page: 3, sort: "rental_listing_date" });
    expect(landFiltersFromSearch(landFiltersToSearch(f))).toEqual(f);
    expect(landSortsFor("sale")[0]).toBe("recently_listed");
  });

  it("builds upstream's /v1/nfts parameters, prices in exact wei", () => {
    const q = landNftsQuery({ ...DEFAULT_LAND_FILTERS, minPrice: "1.5", maxPrice: "2000", search: "genesis", page: 2 });
    expect(q).toMatchObject({ first: 24, skip: 24, sortBy: "recently_listed", isLand: true, isOnSale: true, minPrice: "1500000000000000000", maxPrice: "2000000000000000000000", search: "genesis" });
    expect(landNftsQuery({ ...DEFAULT_LAND_FILTERS, status: "rent", category: "parcel", rentalDays: [7] })).toMatchObject({ category: "parcel", isOnRent: true, rentalDays: ["7"] });
    expect(landNftsQuery({ ...DEFAULT_LAND_FILTERS, status: "all" })).not.toHaveProperty("isOnSale");
  });

  it("calls /shop-api/v1/nfts and reads an index outage as an outage", async () => {
    const client = stubClient({ "GET /shop-api/v1/nfts": { body: { data: [], total: 0 } } });
    expect(await fetchLandNfts(DEFAULT_LAND_FILTERS, { client })).toEqual({ results: [], total: 0 });
    expect(new URL(client.requests[0]!.url).searchParams.get("isLand")).toBe("true");
    const down = stubClient({ "GET /shop-api/v1/nfts": { status: 503, body: { error: "Service Unavailable", message: "nft-indexer is catching up", source: "nft-indexer" } } });
    const err = await fetchLandNfts(DEFAULT_LAND_FILTERS, { client: down }).catch((e) => e);
    expect(indexOutage(err)).toEqual({ message: "nft-indexer is catching up", source: "nft-indexer" });
    expect(indexOutage(new Error("x"))).toBeNull();
  });

  it("reads one token by contract and id, and types its LAND payload", async () => {
    const parcel = { nft: { data: { parcel: { x: "-3", y: "7", description: null, estate: null } } }, order: null, rental: null };
    const client = stubClient({ "GET /shop-api/v1/nfts": { body: { data: [parcel], total: 1 } } });
    const got = await fetchNftResult("0xF87E31492FAF9A91B02EE0DEAAD50D51D56D5D4D", "5", { client });
    expect(new URL(client.requests[0]!.url).searchParams.get("contractAddress")).toBe("0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d");
    expect(landData(got!)).toEqual({ kind: "parcel", x: -3, y: 7, description: null, estate: null });
    expect(landParcels(got!)).toEqual([{ x: -3, y: 7 }]);
  });
});
