// from: shop@94584a5c:app/src/lib/suggestionSeeds.spec.ts
import { describe, expect, it } from "vitest"

import { buildSuggestionSeeds, MAX_SEEDS, seedIdOf, seedsKey } from "./suggestionSeeds"
import { compactEquipped } from "../api"

const item = (contractAddress: string, itemId: string | null) => ({ contractAddress, itemId })

describe("seedIdOf", () => {
  it("lowercases the contract so the id matches what the server keys on", () => {
    expect(seedIdOf(item("0xABC", "3"))).toBe("0xabc-3")
  })

  it("returns null for a row with no item id, which the recommender cannot use", () => {
    expect(seedIdOf(item("0xabc", null))).toBeNull()
  })
})

describe("buildSuggestionSeeds", () => {
  it("orders cart before views, strongest intent first", () => {
    const seeds = buildSuggestionSeeds({
      cart: [item("0xc", "1")],
      recentlyViewed: [item("0xv", "3")]
    })
    expect(seeds).toEqual(["0xc-1", "0xv-3"])
  })

  it("keeps one entry for an item that appears in several sources", () => {
    const seeds = buildSuggestionSeeds({
      cart: [item("0xa", "1")],
      recentlyViewed: [item("0xa", "1")]
    })
    expect(seeds).toEqual(["0xa-1"])
  })

  it("drops rows the recommender cannot key on rather than sending a broken id", () => {
    expect(buildSuggestionSeeds({ cart: [item("0xa", null)], recentlyViewed: [item("0xb", "2")] })).toEqual(["0xb-2"])
  })

  it("caps the list at what the server accepts", () => {
    const many = Array.from({ length: 50 }, (_, i) => item("0xa", String(i)))
    expect(buildSuggestionSeeds({ recentlyViewed: many })).toHaveLength(MAX_SEEDS)
  })

  it("spends the cap on the strongest signals when there are more than fit", () => {
    const views = Array.from({ length: MAX_SEEDS }, (_, i) => item("0xv", String(i)))
    const seeds = buildSuggestionSeeds({ cart: [item("0xc", "1")], recentlyViewed: views })
    expect(seeds[0]).toBe("0xc-1")
  })

  it("returns nothing when the browser knows nothing", () => {
    expect(buildSuggestionSeeds({})).toEqual([])
  })
})

describe("seedsKey", () => {
  it("is the same for the same set in a different order, so re-deriving it does not refetch", () => {
    expect(seedsKey(["b", "a"])).toBe(seedsKey(["a", "b"]))
  })

  it("differs when the set really changes", () => {
    expect(seedsKey(["a"])).not.toBe(seedsKey(["a", "b"]))
  })
})

describe("compactEquipped", () => {
  it("turns a collections-v2 urn into the id the server keys on", () => {
    expect(
      compactEquipped(["urn:decentraland:matic:collections-v2:0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:7"])
    ).toEqual(["0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-7"])
  })

  it("accepts the testnet chain too, so a dev profile behaves like a production one", () => {
    expect(
      compactEquipped(["urn:decentraland:amoy:collections-v2:0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:1"])
    ).toHaveLength(1)
  })

  it("drops base avatars, which every account has and no one can buy", () => {
    expect(compactEquipped(["urn:decentraland:off-chain:base-avatars:eyebrows_00"])).toEqual([])
  })

  it("cuts the request roughly in half, which is the whole point", () => {
    const urns = Array.from(
      { length: 30 },
      (_, i) => `urn:decentraland:matic:collections-v2:0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:${i}`
    )
    const before = urns.join(",").length
    const after = compactEquipped(urns).join(",").length
    expect(after).toBeLessThan(before * 0.6)
  })
})
