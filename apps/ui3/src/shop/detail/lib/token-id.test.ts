// from: shop@94584a5c:app/src/lib/token-id.spec.ts
import { describe, it, expect } from "vitest"
import { decodeTokenId, encodeTokenId, itemIdFromTokenId } from "./token-id"

// tokenId = (itemId << 216) | issuedId. 2^216 is the boundary between the two fields.
const TWO_216 = 1n << 216n

describe("decodeTokenId", () => {
  it("splits a packed tokenId into itemId + issuedId", () => {
    // itemId 5, issuedId 42 → (5 << 216) | 42
    const tokenId = (5n * TWO_216 + 42n).toString()
    expect(decodeTokenId(tokenId)).toEqual({ itemId: "5", issuedId: "42" })
  })

  it("round-trips with encodeTokenId for arbitrary large values", () => {
    const itemId = "123"
    const issuedId = "999999999999"
    const tokenId = encodeTokenId(itemId, issuedId)
    expect(decodeTokenId(tokenId)).toEqual({ itemId, issuedId })
  })

  it("handles item 0, whose tokens have SMALL tokenIds equal to the issuedId (the collision case)", () => {
    // Item 0, copy #7: tokenId = (0 << 216) | 7 = 7 — indistinguishable by magnitude from itemId 7.
    // This is exactly why the app splits routes instead of guessing itemId-vs-tokenId by size.
    const tokenId = encodeTokenId("0", "7")
    expect(tokenId).toBe("7")
    expect(decodeTokenId("7")).toEqual({ itemId: "0", issuedId: "7" })
  })

  it("decodes a realistic large tokenId to its itemId", () => {
    // itemId 1, issuedId 1: (1 << 216) | 1
    const tokenId = (TWO_216 + 1n).toString()
    expect(decodeTokenId(tokenId).itemId).toBe("1")
    expect(decodeTokenId(tokenId).issuedId).toBe("1")
  })

  it("throws on a negative value", () => {
    expect(() => decodeTokenId("-1")).toThrow()
  })
})

describe("itemIdFromTokenId", () => {
  it("returns the decoded itemId", () => {
    expect(itemIdFromTokenId((3n * TWO_216 + 1n).toString())).toBe("3")
  })

  it("returns null for null/empty/malformed input instead of throwing", () => {
    expect(itemIdFromTokenId(null)).toBeNull()
    expect(itemIdFromTokenId(undefined)).toBeNull()
    expect(itemIdFromTokenId("")).toBeNull()
    expect(itemIdFromTokenId("not-a-number")).toBeNull()
  })
})
