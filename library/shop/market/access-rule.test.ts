import { describe, expect, it } from "vitest";

import type { CartLine } from "../model/purchase";
import { isResale, listingTypeFor, needsMarketplace } from "./access-rule";

/** Only the fields the rule reads. */
function line(token: "item" | "token", opts: { category?: string; name?: string; rentalId?: string } = {}): CartLine {
  return {
    asset: { token: token === "item" ? { kind: "item", itemId: "7" } : { kind: "token", tokenId: "42" } },
    listing: { name: opts.name, rentalId: opts.rentalId },
    display: { category: opts.category ?? "wearable" },
  } as unknown as CartLine;
}

describe("which cart lines are the Marketplace's", () => {
  it("a collection item's mint and a NAME registration are the Shop's; a resale, NAME resale and a rental are not", () => {
    expect(needsMarketplace(line("item"))).toBe(false);
    expect(needsMarketplace(line("token", { category: "ens", name: "nacho" }))).toBe(false);
    expect(needsMarketplace(line("token"))).toBe(true);
    expect(needsMarketplace(line("token", { category: "ens" }))).toBe(true);
    expect(needsMarketplace(line("token", { category: "parcel", rentalId: "r1" }))).toBe(true);
    expect(needsMarketplace(line("item", { rentalId: "r1" }))).toBe(true);
  });
});

describe("the listings a visitor is offered", () => {
  it("every kind with the Marketplace, primary sales only without it", () => {
    expect(listingTypeFor(true)).toBeUndefined();
    expect(listingTypeFor(false)).toBe("primary");
  });

  it("a row is a resale when it names a token or says it is secondary", () => {
    expect(isResale({ tokenId: "42" })).toBe(true);
    expect(isResale({ listingType: "secondary" })).toBe(true);
    expect(isResale({ tokenId: null, listingType: "primary" })).toBe(false);
    expect(isResale({})).toBe(false);
  });
});
