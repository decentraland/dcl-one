// from: shop@94584a5c:app/src/lib/popularSearches.spec.ts
import { describe, it, expect } from "vitest"
import { POPULAR_SEARCHES, popularSearchesFor } from "./popularSearches"

describe("when offering popular searches", () => {
  it("should offer all eight to a reader with no recent searches", () => {
    expect(popularSearchesFor([])).toEqual(POPULAR_SEARCHES)
    expect(POPULAR_SEARCHES).toHaveLength(8)
  })

  it("should leave out the ones the reader already searched, whatever the case", () => {
    expect(popularSearchesFor(["Duck", " glow ", "pirate hat"])).toEqual([
      "sword",
      "kimono",
      "wings",
      "dance",
      "hoodie",
      "dress"
    ])
  })
})
