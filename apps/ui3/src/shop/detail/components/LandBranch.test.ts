import { describe, expect, it } from "vitest";

import type { NftResult } from "@catalyst-api/market/NftResult";
import { withOpenOrder } from "./LandBranch";
import { tradeManaPrice } from "./LandTradeBuy";
import type { Trade } from "../../market/lib/trades";

const OWNER = "0x6d8c33438a95b380914066e55fd3dc4bbfc7ddd6";
const ESTATE = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
const NOW = 1_800_000_000_000;

const result = (order: NftResult["order"] = null) => ({ nft: { contractAddress: ESTATE, tokenId: "6178", owner: OWNER }, order, rental: null }) as unknown as NftResult;
const order = (over: Partial<NonNullable<NftResult["order"]>> = {}) =>
  ({
    id: "5ee4f0cc",
    marketplaceAddress: "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
    contractAddress: ESTATE,
    tokenId: "6178",
    owner: OWNER,
    buyer: "",
    price: "160000000000000000000000",
    status: "open",
    expiresAt: 1_842_382_800_000,
    createdAt: 1_773_155_908_588,
    updatedAt: 1_773_155_908_588,
    network: "ETHEREUM",
    chainId: 1,
    issuedId: "6178",
    tradeId: "5ee4f0cc",
    ...over,
  }) as NonNullable<NftResult["order"]>;

describe("withOpenOrder", () => {
  it("attaches the owner's open trade listing that /v1/nfts did not join", () => {
    expect(withOpenOrder(result(), [order()], NOW)?.order?.tradeId).toBe("5ee4f0cc");
  });

  it("keeps an order the NFT already carries", () => {
    const own = order({ id: "legacy", tradeId: null });
    expect(withOpenOrder(result(own), [order()], NOW)?.order?.id).toBe("legacy");
  });

  it("ignores a listing by a previous owner, an expired one, or another token's", () => {
    expect(withOpenOrder(result(), [order({ owner: "0x0000000000000000000000000000000000000001" })], NOW)?.order).toBeNull();
    expect(withOpenOrder(result(), [order({ expiresAt: NOW - 1 })], NOW)?.order).toBeNull();
    expect(withOpenOrder(result(), [order({ tokenId: "1" })], NOW)?.order).toBeNull();
  });
});

describe("tradeManaPrice", () => {
  it("reads the MANA leg of a LAND trade, and nothing from a trade without one", () => {
    const trade = { received: [{ assetType: 1, amount: "160000000000000000000000" }] } as unknown as Trade;
    expect(tradeManaPrice(trade)).toBe(160000000000000000000000n);
    expect(tradeManaPrice({ received: [] } as unknown as Trade)).toBeNull();
  });
});
