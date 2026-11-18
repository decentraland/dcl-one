import { describe, expect, it } from "vitest"
import type { CatalogItem } from "../../discovery/lib/catalogItem"
import { CHAIN } from "../../model/chain"
import { tryOnUrn } from "./tryOn"

const item = (over: Partial<CatalogItem>): CatalogItem =>
  ({ contractAddress: "0xABC", itemId: "7", chainId: CHAIN.POLYGON, ...over }) as CatalogItem

describe("the urn a try-on puts on", () => {
  it("is the item's own urn when it has one", () => {
    expect(tryOnUrn(item({ urn: "urn:decentraland:off-chain:base-avatars:hat", chainId: CHAIN.ETHEREUM }))).toBe(
      "urn:decentraland:off-chain:base-avatars:hat",
    )
  })

  it("is the collections-v2 urn of a Polygon or Amoy item", () => {
    expect(tryOnUrn(item({}))).toBe("urn:decentraland:matic:collections-v2:0xabc:7")
    expect(tryOnUrn(item({ chainId: CHAIN.AMOY }))).toBe("urn:decentraland:amoy:collections-v2:0xabc:7")
  })

  it("is none for an Ethereum item without an urn, or an item without an item id", () => {
    expect(tryOnUrn(item({ chainId: CHAIN.ETHEREUM }))).toBeNull()
    expect(tryOnUrn(item({ itemId: null }))).toBeNull()
  })
})
