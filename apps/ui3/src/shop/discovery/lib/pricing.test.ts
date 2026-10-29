// from: shop@94584a5c:app/src/lib/pricing.spec.ts
import { describe, it, expect } from "vitest"
import { isListingForSale } from "./pricing"

/**
 * The rule this pins: a collection-store mint is FOR SALE without a trade. Reading `tradeId` as the answer
 * is what made the item page say NOT FOR SALE about an item the browse grid was selling — measured on
 * production, `acquisition: "store"`, 48 in stock, 20 MANA, no tradeId and none coming.
 */
describe("when deciding whether a listing is for sale", () => {
  it("should say yes to a trade", () => {
    expect(isListingForSale({ tradeId: "tr-1" })).toBe(true)
  })

  it("should say yes to a store mint with stock, which has no trade", () => {
    expect(isListingForSale({ tradeId: null, acquisition: "store", available: 48 })).toBe(true)
  })

  it("should say no to a store mint that is sold out", () => {
    expect(isListingForSale({ acquisition: "store", available: 0 })).toBe(false)
    expect(isListingForSale({ acquisition: "store" })).toBe(false)
  })

  it("should say no when there is neither a trade nor a mint", () => {
    expect(isListingForSale({ tradeId: null })).toBe(false)
    expect(isListingForSale({})).toBe(false)
  })

  // Stock is a mint's own concern: a TRADE carries its own uses, so a trade row is not gated on it here.
  it("should not read stock for a trade row", () => {
    expect(isListingForSale({ tradeId: "tr-1", acquisition: "trade", available: 0 })).toBe(true)
  })
})
