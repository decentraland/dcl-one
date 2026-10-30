// from: shop@94584a5c:app/src/lib/searchHistory.spec.ts
import { describe, it, expect } from "vitest"
import { searchHistoryMode } from "./searchHistory"

describe("when deciding how a search enters the history", () => {
  it("should push a new query, and a search from somewhere other than the results page", () => {
    expect(searchHistoryMode({ pathname: "/items", search: "?q=hat" }, "/items?q=pirate")).toBe("push")
    expect(searchHistoryMode({ pathname: "/overview", search: "" }, "/items?q=hat")).toBe("push")
    expect(searchHistoryMode({ pathname: "/item/0xabc/1", search: "" }, "/items?q=hat")).toBe("push")
  })

  it("should replace when the destination is exactly the current one, whatever the order of its params", () => {
    expect(searchHistoryMode({ pathname: "/items", search: "?q=hat" }, "/items?q=hat")).toBe("replace")
    expect(searchHistoryMode({ pathname: "/items", search: "?status=all&q=hat" }, "/items?q=hat&status=all")).toBe(
      "replace"
    )
    expect(searchHistoryMode({ pathname: "/items", search: "" }, "/items")).toBe("replace")
  })

  it("should not treat the same query with a different filter as a repetition", () => {
    expect(searchHistoryMode({ pathname: "/items", search: "?q=hat&status=on_sale" }, "/items?q=hat")).toBe("push")
    expect(searchHistoryMode({ pathname: "/items", search: "?q=hat" }, "/items?q=hat&deals=1")).toBe("push")
  })
})
