import { describe, expect, it } from "vitest";

import { toCatalogItem, type RawCollectionItem } from "./catalogItem";

const row = (price: string | undefined): RawCollectionItem =>
  ({ id: "0xc0-1", name: "Hat", contractAddress: "0x00000000000000000000000000000000000000c0", itemId: "1", category: "wearable", network: "MATIC", chainId: 137, price }) as RawCollectionItem;

describe("toCatalogItem", () => {
  it("marks a live mint as buyable from its creator and a closed or missing one as not", () => {
    expect(toCatalogItem(row("1000000000000000000")).hasPrimaryListing).toBe(true);
    expect(toCatalogItem(row("0")).hasPrimaryListing).toBe(false);
    expect(toCatalogItem(row(undefined)).hasPrimaryListing).toBe(false);
  });
});
