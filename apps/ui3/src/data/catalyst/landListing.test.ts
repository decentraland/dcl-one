import { describe, expect, it, vi } from "vitest";
import { describeListing, ESTATE_CONTRACT, fetchLandListing, LAND_CONTRACT, landTokenId, parseParcel } from "./landListing";

const LAND_ORDER = { status: "open", price: "5000000000000000000000", expiresAt: 1919606400 };

function market(routes: Record<string, unknown>) {
  return vi.fn(async (url: string | URL | Request) => {
    const u = new URL(String(url));
    const key = `${u.searchParams.get("contractAddress")}:${u.searchParams.get("tokenId")}`;
    expect(u.pathname).toBe("/market/v1/nfts");
    return new Response(JSON.stringify({ data: key in routes ? [routes[key]] : [] }));
  });
}

describe("landTokenId", () => {
  it("packs x and y like the LAND registry", () => {
    expect(landTokenId(3, -35)).toBe("1361129467683753853853498429727072845789");
    expect(landTokenId(-50, 140)).toBe("115792089237316195423570985008687907836255866319593640866288853636324719067276");
    expect(parseParcel(" 3, -35 ")).toEqual([3, -35]);
    expect(parseParcel("3;12")).toBeNull();
  });
});

describe("fetchLandListing", () => {
  it("reports a parcel's own open order with its price in MANA", async () => {
    const fetchImpl = market({ [`${LAND_CONTRACT}:${landTokenId(3, -35)}`]: { nft: { name: "Dancing Hologram" }, order: LAND_ORDER } });
    const listing = await fetchLandListing("3,-35", { fetchImpl, now: 1_800_000_000_000 });
    expect(listing).toMatchObject({ kind: "parcel", name: "Dancing Hologram", priceMana: 5000, marketplacePath: `/shop/token/${LAND_CONTRACT}/${landTokenId(3, -35)}` });
    expect(describeListing(listing!)).toBe("This parcel is listed for 5,000 MANA. Open on the marketplace.");
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  it("falls back to the estate's order when the parcel sits inside a listed estate", async () => {
    const fetchImpl = market({
      [`${LAND_CONTRACT}:${landTokenId(-58, 78)}`]: { nft: { data: { parcel: { estate: { tokenId: "6504", name: "MEGA Estate" } } } }, order: null },
      [`${ESTATE_CONTRACT}:6504`]: { nft: { name: "MEGA Estate" }, order: { status: "open", price: "1250000000000000000000000" } },
    });
    const listing = await fetchLandListing("-58,78", { fetchImpl });
    expect(listing).toMatchObject({ kind: "estate", tokenId: "6504", priceMana: 1_250_000, marketplacePath: `/shop/token/${ESTATE_CONTRACT}/6504` });
    expect(describeListing(listing!)).toContain("Part of MEGA Estate, listed for 1,250,000 MANA");
  });

  it("keeps fractional prices instead of truncating them", async () => {
    const fetchImpl = market({ [`${LAND_CONTRACT}:${landTokenId(3, -35)}`]: { nft: { name: "Parcel" }, order: { ...LAND_ORDER, price: "9393030000000000000000" } } });
    const listing = await fetchLandListing("3,-35", { fetchImpl, now: 1_800_000_000_000 });
    expect(listing?.priceMana).toBe(9393.03);
    expect(describeListing(listing!)).toBe("This parcel is listed for 9,393.03 MANA. Open on the marketplace.");
    const tiny = market({ [`${LAND_CONTRACT}:${landTokenId(3, -35)}`]: { nft: {}, order: { ...LAND_ORDER, price: "500000000000000000" } } });
    expect((await fetchLandListing("3,-35", { fetchImpl: tiny, now: 1_800_000_000_000 }))?.priceMana).toBe(0.5);
  });

  it("ignores expired, cancelled and missing orders", async () => {
    const expired = market({ [`${LAND_CONTRACT}:${landTokenId(3, -35)}`]: { nft: {}, order: { ...LAND_ORDER, expiresAt: 1_600_000_000 } } });
    expect(await fetchLandListing("3,-35", { fetchImpl: expired, now: 1_800_000_000_000 })).toBeNull();
    const cancelled = market({ [`${LAND_CONTRACT}:${landTokenId(3, -35)}`]: { nft: {}, order: { ...LAND_ORDER, status: "cancelled" } } });
    expect(await fetchLandListing("3,-35", { fetchImpl: cancelled })).toBeNull();
    const none = market({});
    expect(await fetchLandListing("3,-35", { fetchImpl: none })).toBeNull();
    expect(none).toHaveBeenCalledTimes(1);
  });
});
