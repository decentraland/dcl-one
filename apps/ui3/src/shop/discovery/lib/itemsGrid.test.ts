import { describe, expect, it } from "vitest";

import { itemsFilterDefaults, itemsGrid } from "./itemsGrid";

describe("itemsGrid", () => {
  it("keeps the price range on every status that has prices, not only on the on-sale grid", () => {
    for (const status of ["on_sale", "all"]) {
      const { key } = itemsGrid({ ...itemsFilterDefaults("", false), status, priceMin: "5", priceMax: "20" }, "");
      expect(key?.[1], status).toMatchObject({ minPriceCredits: 5, maxPriceCredits: 20 });
    }
    // Nothing not for sale has a price: a range there would empty the grid.
    const { key } = itemsGrid({ ...itemsFilterDefaults("", false), status: "not_for_sale", priceMin: "5", priceMax: "20" }, "");
    expect(key?.[1]).toMatchObject({ minPriceCredits: undefined, maxPriceCredits: undefined });
  });

  it("asks for items not on sale when the status says so", () => {
    const { key, isUnified } = itemsGrid({ ...itemsFilterDefaults("", false), status: "not_for_sale" }, "");
    expect(isUnified).toBe(false);
    expect(key).toEqual(["catalog-items", expect.objectContaining({ isOnSale: false })]);
  });
});
