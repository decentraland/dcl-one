import { describe, expect, it } from "vitest"
import { ADDRESS, MAX_PURCHASE_UNITS, sameItem, UINT, unitsOf } from "./lines"
import { MAX_CREDIT_LINES_PER_GROUP } from "../model/settlement"

describe("a purchase line's rules", () => {
  it("names the same item by contract and item id only", () => {
    expect(sameItem({ contract: "0xa", itemId: "1" }, { contract: "0xa", itemId: "1" })).toBe(true)
    expect(sameItem({ contract: "0xa", itemId: "1" }, { contract: "0xa", itemId: "2" })).toBe(false)
    expect(sameItem({ contract: "0xa", itemId: "1" }, { contract: "0xA", itemId: "1" })).toBe(false)
  })

  it("counts the units of all lines, zero for none", () => {
    expect(unitsOf([{ quantity: 2 }, { quantity: 3 }])).toBe(5)
    expect(unitsOf([])).toBe(0)
  })

  it("takes an address in any case and a decimal uint256 without leading zeros", () => {
    expect(ADDRESS.test("0x" + "aB".repeat(20))).toBe(true)
    expect(ADDRESS.test("0x" + "a".repeat(39))).toBe(false)
    expect(UINT.test("0")).toBe(true)
    expect(UINT.test("012")).toBe(false)
    expect(UINT.test("9".repeat(79))).toBe(false)
  })

  it("caps a purchase at one Credits group", () => {
    expect(MAX_PURCHASE_UNITS).toBe(MAX_CREDIT_LINES_PER_GROUP)
  })
})
