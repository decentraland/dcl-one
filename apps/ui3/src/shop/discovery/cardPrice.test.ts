
import { describe, expect, it } from "vitest";

import { manaWei } from "../model/money";
import { SIGNED_OUT, web2, web3 } from "../payment/fixtures";
import { shopListingToItem, unifiedListingToItem } from "./api";
import { cardEligibility, cardPrice } from "./cardPrice";
import { L1_MINT, PRIMARY_WEARABLE, RESALE_WEARABLE, STORE_EMOTE, UNPRICED_WEARABLE, saleWearable } from "./fixtures";
import type { CatalogItem } from "./lib/catalogItem";

const SESSIONS = [SIGNED_OUT, web2(), web3()] as const;

describe("an eligible primary card", () => {
  it.each(SESSIONS)("shows credits as the price for %o", (session) => {
    const price = cardPrice(unifiedListingToItem(PRIMARY_WEARABLE), session);
    expect(price).toEqual({ kind: "credits", credits: 12, compareAt: null, mana: null });
  });

  it("adds the exact MANA of a MANA-priced mint only for a web3 wallet, credits still first", () => {
    const item = unifiedListingToItem(STORE_EMOTE);
    expect(cardPrice(item, web3())).toEqual({ kind: "credits", credits: 21, compareAt: null, mana: manaWei(4_020_000_000_000_000_000n) });
    expect(cardPrice(item, web2())).toMatchObject({ kind: "credits", mana: null });
    expect(cardPrice(item, SIGNED_OUT)).toMatchObject({ kind: "credits", mana: null });
  });

  it("keeps a running sale's compare-at when creator sales are on", () => {
    (window as Window & { __SHOP_FLAGS__?: { creatorSales?: boolean } }).__SHOP_FLAGS__ = { creatorSales: true };
    try {
      expect(cardPrice(unifiedListingToItem(saleWearable()), web2())).toMatchObject({ kind: "credits", credits: 14, compareAt: 20 });
    } finally {
      delete (window as Window & { __SHOP_FLAGS__?: unknown }).__SHOP_FLAGS__;
    }
  });

  it("states a missing credit price as unavailable and never moves to MANA", () => {
    for (const session of SESSIONS) expect(cardPrice(unifiedListingToItem(UNPRICED_WEARABLE), session).kind).toBe("creditsUnavailable");
    const mintWithoutCredits = { ...unifiedListingToItem(STORE_EMOTE), priceCredits: 0 };
    expect(cardPrice(mintWithoutCredits, web3())).toEqual({ kind: "creditsUnavailable", mana: manaWei(4_020_000_000_000_000_000n) });
  });
});

describe("a MANA-only card", () => {
  it.each(SESSIONS)("a resale never shows its feed credit figure (%o)", (session) => {
    const item = unifiedListingToItem(RESALE_WEARABLE);
    expect(item.priceCredits).toBe(10);
    const price = cardPrice(item, session);
    expect(price.kind).toBe("manaAtCheckout");
    expect(price).toMatchObject({ reasons: ["secondarySale"] });
  });

  it("a creator reselling a minted token is still a resale", () => {
    const item: CatalogItem = { ...unifiedListingToItem(RESALE_WEARABLE), seller: PRIMARY_WEARABLE.creator };
    expect(cardPrice(item, web3()).kind).toBe("manaAtCheckout");
  });

  it("a row the feed labels secondary is judged a resale even without a token id", () => {
    const item: CatalogItem = { ...unifiedListingToItem(PRIMARY_WEARABLE), listingType: "secondary" };
    expect(cardPrice(item, web3()).kind).toBe("manaAtCheckout");
  });

  it("a resale with an exact MANA order shows that MANA", () => {
    const item: CatalogItem = { ...unifiedListingToItem(RESALE_WEARABLE), manaWei: "2000000000000000000" };
    expect(cardPrice(item, web2())).toEqual({ kind: "mana", mana: manaWei(2_000_000_000_000_000_000n), reasons: ["secondarySale"] });
  });

  it("an Ethereum mint is MANA only: credits settle on Polygon", () => {
    const price = cardPrice(unifiedListingToItem(L1_MINT), web3());
    expect(price).toEqual({ kind: "mana", mana: manaWei(12_000_000_000_000_000_000n), reasons: ["creditsChainUnsupported"] });
  });

  it("LAND and anything not a Decentraland wearable or emote is MANA only", () => {
    const parcel: CatalogItem = { ...unifiedListingToItem(RESALE_WEARABLE), category: "parcel", manaWei: "4000000000000000000000" };
    expect(cardPrice(parcel, web2())).toMatchObject({ kind: "mana", reasons: expect.arrayContaining(["landOrRental", "secondarySale"]) });
    const other: CatalogItem = { ...unifiedListingToItem(PRIMARY_WEARABLE), category: "something" };
    expect(cardPrice(other, web3())).toMatchObject({ kind: "manaAtCheckout", reasons: ["notDecentralandItem"] });
  });
});

describe("cards with nothing to buy", () => {
  it("a primary with no stock left is sold out", () => {
    expect(cardPrice({ ...unifiedListingToItem(PRIMARY_WEARABLE), available: 0 }, web3()).kind).toBe("soldOut");
  });

  it("a catalogue row not on sale is not for sale", () => {
    const item = { ...shopListingToItem(PRIMARY_WEARABLE), tradeId: undefined, priceCredits: 0, onSale: false };
    expect(cardPrice(item, web3()).kind).toBe("notForSale");
  });
});

describe("the eligibility a card reads", () => {
  it("is null for a row that names no asset", () => {
    expect(cardEligibility({ ...unifiedListingToItem(PRIMARY_WEARABLE), contractAddress: "nope" }, web2())).toBeNull();
    expect(cardEligibility({ ...unifiedListingToItem(PRIMARY_WEARABLE), itemId: null }, web2())).toBeNull();
  });
});
