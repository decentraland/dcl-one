// from: shop@94584a5c:app/src/lib/pagedRail.spec.ts
import { describe, it, expect } from "vitest"
import { railPageCount, railPageFromScroll } from "./pagedRail"

describe("when counting the pages of a rail", () => {
  it("should report a single page when nothing overflows", () => {
    expect(railPageCount(1000, 1000)).toBe(1)
    expect(railPageCount(600, 1000)).toBe(1)
  })

  it("should count a partial overflow as one extra page", () => {
    expect(railPageCount(1200, 1000)).toBe(2)
    expect(railPageCount(2000, 1000)).toBe(2)
    expect(railPageCount(2400, 1000)).toBe(3)
  })

  it("should not divide by an unmeasured viewport", () => {
    expect(railPageCount(1200, 0)).toBe(1)
  })
})

describe("when deriving the current page from a scroll offset", () => {
  it("should report the first page at rest", () => {
    expect(railPageFromScroll(0, 2400, 1000)).toBe(0)
  })

  // The regression this guards: 6 cards at 5-per-view can only scroll 200px, so dividing by the
  // viewport width would round to page 0 and the end arrow/dot could never light up.
  it("should reach the last page even when it is only a sliver of a viewport", () => {
    expect(railPageFromScroll(200, 1200, 1000)).toBe(1)
  })

  it("should map the middle of the extent onto a middle page", () => {
    expect(railPageFromScroll(700, 2400, 1000)).toBe(1)
    expect(railPageFromScroll(1400, 2400, 1000)).toBe(2)
  })

  it("should stay on the first page when there is nothing to scroll", () => {
    expect(railPageFromScroll(0, 800, 1000)).toBe(0)
    expect(railPageFromScroll(50, 1000, 1000)).toBe(0)
  })

  it("should clamp an over-scroll (rubber-banding) to the ends", () => {
    expect(railPageFromScroll(-40, 2400, 1000)).toBe(0)
    expect(railPageFromScroll(9999, 2400, 1000)).toBe(2)
  })
})
