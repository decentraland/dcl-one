import { expect, it, vi } from "vitest";

import { fetchCatalog } from "./index";

it("passes the positive-price filter to the backend before it paginates the catalog", async () => {
  const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(Response.json({ data: [], total: 0 }));
  // `minPrice` is MANA (the legacy marketplace API's unit), passed through as given.
  await fetchCatalog({ first: 8, isOnSale: true, minPrice: "2.5" }, { base: "https://market.test", fetchImpl });
  const url = new URL(String(fetchImpl.mock.calls[0][0]));
  expect(url.pathname).toBe("/marketplace-api/v1/catalog");
  expect(Object.fromEntries(url.searchParams)).toEqual({ first: "8", isOnSale: "true", minPrice: "2.5" });
});
