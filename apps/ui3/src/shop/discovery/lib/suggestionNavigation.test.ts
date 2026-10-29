// from: shop@94584a5c:app/src/lib/suggestionNavigation.spec.ts
import { describe, it, expect } from "vitest"
import { NO_ACTIVE_ROW, nextActiveIndex, searchKeyAction, suggestionRowId } from "./suggestionNavigation"

describe("when moving through the suggestions with the keyboard", () => {
  it("should step down from nothing to the first row, then row by row, wrapping at the end", () => {
    expect(nextActiveIndex(NO_ACTIVE_ROW, 3, "ArrowDown")).toBe(0)
    expect(nextActiveIndex(0, 3, "ArrowDown")).toBe(1)
    expect(nextActiveIndex(2, 3, "ArrowDown")).toBe(0)
  })

  it("should step up from nothing to the last row, then row by row, wrapping at the start", () => {
    expect(nextActiveIndex(NO_ACTIVE_ROW, 3, "ArrowUp")).toBe(2)
    expect(nextActiveIndex(2, 3, "ArrowUp")).toBe(1)
    expect(nextActiveIndex(0, 3, "ArrowUp")).toBe(2)
  })

  it("should jump to the ends with Home and End", () => {
    expect(nextActiveIndex(1, 3, "Home")).toBe(0)
    expect(nextActiveIndex(1, 3, "End")).toBe(2)
  })

  it("should never activate anything when there are no rows", () => {
    expect(nextActiveIndex(NO_ACTIVE_ROW, 0, "ArrowDown")).toBe(NO_ACTIVE_ROW)
    expect(nextActiveIndex(NO_ACTIVE_ROW, 0, "End")).toBe(NO_ACTIVE_ROW)
  })

  it("should leave other keys alone", () => {
    expect(nextActiveIndex(1, 3, "Enter")).toBeNull()
    expect(nextActiveIndex(1, 3, "a")).toBeNull()
  })
})

describe("when a key is pressed in the search box", () => {
  const open = { open: true, activeIndex: NO_ACTIVE_ROW, count: 3 }
  const navigating = { open: true, activeIndex: 1, count: 3 }
  const closed = { open: false, activeIndex: NO_ACTIVE_ROW, count: 0 }

  it("should move through the rows with the arrows while the panel is open, and not otherwise", () => {
    expect(searchKeyAction({ key: "ArrowDown" }, open)).toEqual({ type: "move", index: 0 })
    expect(searchKeyAction({ key: "ArrowUp" }, navigating)).toEqual({ type: "move", index: 0 })
    expect(searchKeyAction({ key: "ArrowDown" }, closed)).toBeNull()
  })

  it("should leave Home and End to the caret until a row is active", () => {
    expect(searchKeyAction({ key: "Home" }, open)).toBeNull()
    expect(searchKeyAction({ key: "End" }, open)).toBeNull()
    expect(searchKeyAction({ key: "Home" }, navigating)).toEqual({ type: "move", index: 0 })
    expect(searchKeyAction({ key: "End" }, navigating)).toEqual({ type: "move", index: 2 })
  })

  it("should activate the active row on Enter, or submit the search when none is", () => {
    expect(searchKeyAction({ key: "Enter" }, navigating)).toEqual({ type: "activate" })
    expect(searchKeyAction({ key: "Enter" }, open)).toEqual({ type: "submit" })
    expect(searchKeyAction({ key: "Enter" }, closed)).toEqual({ type: "submit" })
  })

  it("should close an open panel on Escape and clear the box on the next one", () => {
    expect(searchKeyAction({ key: "Escape" }, navigating)).toEqual({ type: "close" })
    expect(searchKeyAction({ key: "Escape" }, closed)).toEqual({ type: "clear" })
  })

  it("should leave the text alone under a modifier or while an IME composes", () => {
    expect(searchKeyAction({ key: "ArrowDown", shiftKey: true }, navigating)).toBeNull()
    expect(searchKeyAction({ key: "Home", ctrlKey: true }, navigating)).toBeNull()
    expect(searchKeyAction({ key: "Enter", metaKey: true }, navigating)).toBeNull()
    expect(searchKeyAction({ key: "ArrowUp", altKey: true }, navigating)).toBeNull()
    expect(searchKeyAction({ key: "Enter", isComposing: true }, navigating)).toBeNull()
    expect(searchKeyAction({ key: "Process" }, navigating)).toBeNull()
  })

  it("should let other keys through untouched", () => {
    expect(searchKeyAction({ key: "a" }, navigating)).toBeNull()
    expect(searchKeyAction({ key: "Tab" }, navigating)).toBeNull()
  })
})

describe("when giving a row its DOM id", () => {
  it("should never let two different keys share one, whatever characters they hold", () => {
    expect(suggestionRowId("recent", "a b")).not.toBe(suggestionRowId("recent", "a-b"))
    expect(suggestionRowId("recent", "máscara")).not.toBe(suggestionRowId("recent", "mascara"))
    expect(suggestionRowId("recent", "猫")).not.toBe(suggestionRowId("recent", "犬"))
    expect(suggestionRowId("creator", "0xabc")).not.toBe(suggestionRowId("collection", "0xabc"))
  })

  it("should be the same id for the same row, and safe for the DOM", () => {
    expect(suggestionRowId("item", "0xAbC-1")).toBe(suggestionRowId("item", "0xAbC-1"))
    expect(suggestionRowId("recent", "pirate hat!")).toMatch(/^search-row-recent-[0-9a-f]+$/)
  })
})
