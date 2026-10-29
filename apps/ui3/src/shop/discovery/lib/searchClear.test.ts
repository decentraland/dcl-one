// from: shop@94584a5c:app/src/lib/searchClear.spec.ts
import { describe, it, expect } from "vitest"
import { clearedSearchUrl } from "./searchClear"

describe("when clearing the search box", () => {
  it("should drop the query from the results page and keep the other filters", () => {
    expect(clearedSearchUrl("/shop/items", "?q=Nebula&status=not_for_sale")).toBe("/shop/items?status=not_for_sale")
    expect(clearedSearchUrl("/shop/items", "?q=Nebula")).toBe("/shop/items")
  })

  it("should stay put anywhere else, and on the results page with no query", () => {
    expect(clearedSearchUrl("/overview", "?q=Nebula")).toBeNull()
    expect(clearedSearchUrl("/shop/item/0xabc/0", "")).toBeNull()
    expect(clearedSearchUrl("/shop/items", "?status=not_for_sale")).toBeNull()
  })
})
