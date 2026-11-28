import { describe, expect, it } from "vitest";

import { itemsFilterDefaults, itemsGrid, itemsQuery } from "./itemsGrid";
import { readUrlFilters } from "./urlFilters";

describe("itemsGrid", () => {
  it("restores a search from a shared URL with empty list entries and an unknown sort", () => {
    const params = new URLSearchParams("q=+Hat+&rarities=rare,,epic,&smart=1&sort=unknown&from=shared");
    const { rawQuery, q } = itemsQuery(params);
    expect(rawQuery).toBe("Hat");
    const defaults = itemsFilterDefaults(q, false);
    const filters = readUrlFilters(params, defaults);
    expect(filters.rarities).toEqual(["rare", "epic"]);
    expect(filters).not.toHaveProperty("from");
    expect(itemsGrid(filters, q, false).key).toEqual(["catalog-items", expect.objectContaining({
      search: "hat", rarities: ["rare", "epic"], isWearableSmart: undefined, sortBy: "relevance",
    })]);
    expect(defaults.rarities).toEqual([]);
  });

  it("keeps the price range on every status that has prices, not only on the on-sale grid", () => {
    for (const status of ["on_sale", "all"]) {
      const { key } = itemsGrid({ ...itemsFilterDefaults("", false), status, priceMin: "5", priceMax: "20" }, "", false);
      expect(key?.[1], status).toMatchObject({ minPriceCredits: 5, maxPriceCredits: 20 });
    }
    // Nothing not for sale has a price: a range there would empty the grid.
    const { key } = itemsGrid({ ...itemsFilterDefaults("", false), status: "not_for_sale", priceMin: "5", priceMax: "20" }, "", false);
    expect(key?.[1]).toMatchObject({ minPriceCredits: undefined, maxPriceCredits: undefined });
  });

  it("asks for items not on sale when the status says so", () => {
    const { key, isUnified } = itemsGrid({ ...itemsFilterDefaults("", false), status: "not_for_sale" }, "", false);
    expect(isUnified).toBe(false);
    expect(key).toEqual(["catalog-items", expect.objectContaining({ isOnSale: false })]);
  });

  it("without the Marketplace a ?listingType= selects nothing and the read asks for primary sales", () => {
    const resale = { ...itemsFilterDefaults("", false), status: "on_sale", listingType: "secondary" };
    const off = itemsGrid(resale, "", false);
    expect(off.listingType).toBeUndefined();
    expect(off.key?.[1]).toMatchObject({ listingType: "primary" });
    const on = itemsGrid(resale, "", false, { marketplace: true });
    expect(on.listingType).toBe("secondary");
    expect(on.key?.[1]).toMatchObject({ listingType: "secondary" });
  });
});
