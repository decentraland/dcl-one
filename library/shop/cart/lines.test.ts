import { beforeEach, describe, expect, it } from "vitest"
import { ADDRESS, MAX_PURCHASE_UNITS, sameItem, UINT, unitsOf } from "./lines"
import { MAX_CREDIT_LINES_PER_GROUP } from "../model/settlement"

describe("sameItem", () => {
  let line: { contract: string; itemId: string }
  let other: { contract: string; itemId: string }

  beforeEach(() => {
    line = { contract: "0xa", itemId: "1" }
  })

  describe("when the contract and item id match", () => {
    beforeEach(() => {
      other = { contract: "0xa", itemId: "1" }
    })

    it("should name the same item", () => {
      expect(sameItem(line, other)).toBe(true)
    })
  })

  describe("when the item id differs", () => {
    beforeEach(() => {
      other = { contract: "0xa", itemId: "2" }
    })

    it("should name another item", () => {
      expect(sameItem(line, other)).toBe(false)
    })
  })

  describe("when the contract differs only in case", () => {
    beforeEach(() => {
      other = { contract: "0xA", itemId: "1" }
    })

    it("should name another item", () => {
      expect(sameItem(line, other)).toBe(false)
    })
  })
})

describe("unitsOf", () => {
  let lines: { quantity: number }[]

  describe("when there are lines", () => {
    beforeEach(() => {
      lines = [{ quantity: 2 }, { quantity: 3 }]
    })

    it("should count the units of all lines", () => {
      expect(unitsOf(lines)).toBe(5)
    })
  })

  describe("when there are no lines", () => {
    beforeEach(() => {
      lines = []
    })

    it("should count zero", () => {
      expect(unitsOf(lines)).toBe(0)
    })
  })
})

describe("ADDRESS", () => {
  describe("when the address mixes upper and lower case", () => {
    it("should accept it", () => {
      expect(ADDRESS.test("0x" + "aB".repeat(20))).toBe(true)
    })
  })

  describe("when the address is one hex digit short", () => {
    it("should refuse it", () => {
      expect(ADDRESS.test("0x" + "a".repeat(39))).toBe(false)
    })
  })
})

describe("UINT", () => {
  describe("when the value is zero", () => {
    it("should accept it", () => {
      expect(UINT.test("0")).toBe(true)
    })
  })

  describe("when the value has a leading zero", () => {
    it("should refuse it", () => {
      expect(UINT.test("012")).toBe(false)
    })
  })

  describe("when the value has more digits than a uint256", () => {
    it("should refuse it", () => {
      expect(UINT.test("9".repeat(79))).toBe(false)
    })
  })
})

describe("MAX_PURCHASE_UNITS", () => {
  it("should cap a purchase at one Credits group", () => {
    expect(MAX_PURCHASE_UNITS).toBe(MAX_CREDIT_LINES_PER_GROUP)
  })
})
