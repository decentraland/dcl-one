import { describe, expect, it } from "vitest";

import { itemsFilterDefaults, itemsGrid } from "./itemsGrid";

describe("itemsGrid", () => {
  it("keeps the price range on every status, not only on the on-sale grid", () => {
    for (const status of ["on_sale", "all", "not_for_sale"]) {
      const { key } = itemsGrid({ ...itemsFilterDefaults("", false), status, priceMin: "5", priceMax: "20" }, "");
      expect(key?.[1], status).toMatchObject({ minPriceCredits: 5, maxPriceCredits: 20 });
    }
  });

  it("asks for items not on sale when the status says so", () => {
    const { key, isUnified } = itemsGrid({ ...itemsFilterDefaults("", false), status: "not_for_sale" }, "");
    expect(isUnified).toBe(false);
    expect(key).toEqual(["catalog-items", expect.objectContaining({ isOnSale: false })]);
  });
});
