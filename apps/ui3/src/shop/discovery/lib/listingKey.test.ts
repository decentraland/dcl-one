// from: shop@94584a5c:app/src/lib/listingKey.spec.ts
import { describe, it, expect } from "vitest"
import { listingKey } from "./listingKey"
import type { UnifiedListing } from "../api"

// Minimal factory — listingKey only reads source/tradeId/contractAddress/itemId/tokenId.
function listing(over: Partial<UnifiedListing>): UnifiedListing {
  return {
    // Defaults to a trade: that is what every row was before CollectionStore mints entered the feed, and
    // the cases below are all about trades. A store case overrides it explicitly.
    acquisition: "trade",
    id: "",
    name: "",
    creator: "",
    contractAddress: "0xitem",
    itemId: "0",
    category: "wearable",
    rarity: "common",
    network: "MATIC",
    chainId: 137,
    thumbnail: "",
    priceCredits: 0,
    gender: null,
    isSmart: false,
    source: "native",
    manaWei: null,
    ...over
  }
}

describe("listingKey", () => {
  it("keys by source + trade id when a trade id is present", () => {
    expect(listingKey(listing({ source: "native", tradeId: "trade-1" }))).toBe("native:trade-1")
    expect(listingKey(listing({ source: "legacy", tradeId: "trade-2" }))).toBe("legacy:trade-2")
  })

  it("falls back to contractAddress + token/item identity when the trade id is empty", () => {
    const key = listingKey(listing({ tradeId: "", contractAddress: "0xabc", itemId: "7", tokenId: undefined }))
    expect(key).toBe("native:0xabc-7")
  })

  it("stays UNIQUE for the same item listed under both native and legacy sources", () => {
    // The same underlying item can surface under both liquidity sources in the merged feed; the
    // source prefix keeps their keys distinct so React never reconciles one card onto the other.
    const native = listingKey(listing({ source: "native", tradeId: "", contractAddress: "0xabc", itemId: "7" }))
    const legacy = listingKey(listing({ source: "legacy", tradeId: "", contractAddress: "0xabc", itemId: "7" }))
    expect(native).not.toBe(legacy)
  })

  it("produces no duplicate keys across a mixed feed", () => {
    const items = [
      listing({ source: "native", tradeId: "a" }),
      listing({ source: "legacy", tradeId: "b" }),
      listing({ source: "native", tradeId: "", contractAddress: "0x1", tokenId: "10", itemId: null }),
      listing({ source: "native", tradeId: "", contractAddress: "0x1", tokenId: "11", itemId: null })
    ]
    const keys = items.map(listingKey)
    expect(new Set(keys).size).toBe(keys.length)
  })

  it("is stable for the same listing across re-fetches", () => {
    const first = listingKey(listing({ source: "legacy", tradeId: "trade-9" }))
    const again = listingKey(listing({ source: "legacy", tradeId: "trade-9", priceCredits: 999 }))
    expect(first).toBe(again)
  })
})
