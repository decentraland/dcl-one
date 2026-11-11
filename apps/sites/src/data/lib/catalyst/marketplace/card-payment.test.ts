import { describe, expect, it } from "vitest";

import { collectiblePriceText, toCollectibleCard, type CatalogItem } from "./index";

const COLLECTION = "0x00000000000000000000000000000000000000c0";

function item(overrides: Partial<CatalogItem> = {}): CatalogItem {
  return {
    id: `${COLLECTION}-7`,
    itemId: "7",
    name: "Rainbow Hat",
    thumbnail: null,
    url: null,
    urn: null,
    category: "wearable",
    contractAddress: COLLECTION,
    rarity: "epic",
    available: 10,
    isOnSale: true,
    creator: null,
    network: "MATIC",
    chainId: 137,
    price: "2300000000000000000",
    minListingPrice: "1234567890000000000",
    createdAt: null,
    firstListedAt: null,
    data: {},
    ...overrides,
  };
}

describe("a website Shop card's price", () => {
  it("an eligible primary wearable shows its credit price", () => {
    expect(collectiblePriceText(toCollectibleCard(item(), "12"))).toBe("12 credits");
  });

  it("an eligible primary wearable whose quote failed says so, never a MANA fallback", () => {
    const card = toCollectibleCard(item(), null);
    expect(card.payment).toEqual({ kind: "creditsUnavailable" });
    expect(collectiblePriceText(card)).toBe("Credits price unavailable");
  });

  it("an item that mints on Ethereum is MANA only, in exact units", () => {
    expect(collectiblePriceText(toCollectibleCard(item({ chainId: 1, network: "ETHEREUM", price: "1234567891000000000" }), "12"))).toBe("1.234567891 MANA");
  });

  it("an item with only resales is MANA only, at the resale's exact price, never with a credit figure", () => {
    expect(collectiblePriceText(toCollectibleCard(item({ isOnSale: false, price: "0" }), "12"))).toBe("1.23456789 MANA");
  });

  it("an unpriced row shows nothing", () => {
    expect(collectiblePriceText(toCollectibleCard(item({ isOnSale: false, price: "0", minListingPrice: null }), null))).toBeNull();
  });
});
