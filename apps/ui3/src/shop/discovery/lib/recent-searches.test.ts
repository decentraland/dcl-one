// from: shop@94584a5c:app/src/lib/recent-searches.spec.ts
import { beforeEach, describe, expect, it } from "vitest"
import { clearRecentSearches, getRecentSearches, recordSearch, removeRecentSearch } from "./recent-searches"

describe("recent-searches", () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it("returns an empty list when nothing is stored", () => {
    expect(getRecentSearches()).toEqual([])
  })

  it("records a search, most-recent-first", () => {
    recordSearch("hat")
    recordSearch("shoes")
    expect(getRecentSearches()).toEqual(["shoes", "hat"])
  })

  it("trims whitespace and ignores empty queries", () => {
    recordSearch("  dragon  ")
    recordSearch("   ")
    recordSearch("")
    expect(getRecentSearches()).toEqual(["dragon"])
  })

  it("dedupes case-insensitively and moves the match to the front", () => {
    recordSearch("Hat")
    recordSearch("shoes")
    recordSearch("HAT")
    expect(getRecentSearches()).toEqual(["HAT", "shoes"])
  })

  it("caps the list at the maximum size", () => {
    for (let i = 0; i < 12; i++) recordSearch(`q${i}`)
    const list = getRecentSearches()
    expect(list).toHaveLength(8)
    // Newest kept, oldest dropped.
    expect(list[0]).toBe("q11")
    expect(list).not.toContain("q0")
  })

  it("removes a single entry (case-insensitively)", () => {
    recordSearch("hat")
    recordSearch("shoes")
    removeRecentSearch("HAT")
    expect(getRecentSearches()).toEqual(["shoes"])
  })

  it("clears all entries", () => {
    recordSearch("hat")
    recordSearch("shoes")
    clearRecentSearches()
    expect(getRecentSearches()).toEqual([])
  })

  it("survives malformed stored data without throwing", () => {
    localStorage.setItem("shop:recent-searches", "not json")
    expect(getRecentSearches()).toEqual([])
  })

  it("ignores non-string entries in stored data", () => {
    localStorage.setItem("shop:recent-searches", JSON.stringify(["ok", 42, null, "fine"]))
    expect(getRecentSearches()).toEqual(["ok", "fine"])
  })
})
