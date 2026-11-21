import { beforeEach, describe, expect, it } from "vitest"
import type { CatalogItem } from "../../discovery/lib/catalogItem"
import { CHAIN } from "../../model/chain"
import { tryOnUrn } from "./tryOn"

const item = (over: Partial<CatalogItem>): CatalogItem =>
  ({ contractAddress: "0xABC", itemId: "7", chainId: CHAIN.POLYGON, ...over }) as CatalogItem

describe("tryOnUrn", () => {
  let catalogItem: CatalogItem

  describe("when the item has its own urn", () => {
    beforeEach(() => {
      catalogItem = item({ urn: "urn:decentraland:off-chain:base-avatars:hat", chainId: CHAIN.ETHEREUM })
    })

    it("should return the item's own urn", () => {
      expect(tryOnUrn(catalogItem)).toBe("urn:decentraland:off-chain:base-avatars:hat")
    })
  })

  describe("when the item is on Polygon without an urn", () => {
    beforeEach(() => {
      catalogItem = item({})
    })

    it("should return its matic collections-v2 urn", () => {
      expect(tryOnUrn(catalogItem)).toBe("urn:decentraland:matic:collections-v2:0xabc:7")
    })
  })

  describe("when the item is on Amoy without an urn", () => {
    beforeEach(() => {
      catalogItem = item({ chainId: CHAIN.AMOY })
    })

    it("should return its amoy collections-v2 urn", () => {
      expect(tryOnUrn(catalogItem)).toBe("urn:decentraland:amoy:collections-v2:0xabc:7")
    })
  })

  describe("when the item is on Ethereum without an urn", () => {
    beforeEach(() => {
      catalogItem = item({ chainId: CHAIN.ETHEREUM })
    })

    it("should return null", () => {
      expect(tryOnUrn(catalogItem)).toBeNull()
    })
  })

  describe("when the item has no item id", () => {
    beforeEach(() => {
      catalogItem = item({ itemId: null })
    })

    it("should return null", () => {
      expect(tryOnUrn(catalogItem)).toBeNull()
    })
  })
})
