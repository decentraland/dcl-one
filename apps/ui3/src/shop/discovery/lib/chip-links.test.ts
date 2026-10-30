// from: shop@94584a5c:app/src/lib/chip-links.spec.ts
import { describe, expect, it } from "vitest"

import { categoryHref, rarityHref, smartHref } from "./chip-links"

/**
 * These URLs have to be exactly what `useUrlFilters` would have written had the reader set the filter by
 * hand — a chip that lands on a grid ignoring half its query is worse than one that does not link at
 * all, because it looks like it worked.
 */
describe("chip links", () => {
  describe("when the item has a rarity", () => {
    it("should filter the browse page by it, lowercased as the filter state spells it", () => {
      expect(rarityHref("LEGENDARY")).toBe("/shop/items?rarities=legendary")
    })

    it("should not link when the item carries none", () => {
      expect(rarityHref(undefined)).toBeNull()
    })
  })

  describe("when the item occupies a wearable slot", () => {
    it("should select the slot within its section, so the sidebar shows what is selected", () => {
      expect(categoryHref({ category: "wearable", wearableCategory: "eyebrows" })).toBe(
        "/shop/items?category=wearable&subCategory=Eyebrows"
      )
    })

    it("should resolve a multi-word slot to the key the filter state holds", () => {
      expect(categoryHref({ category: "wearable", wearableCategory: "top_head" })).toBe(
        "/shop/items?category=wearable&subCategory=Top%20Head"
      )
    })

    it("should fall back to the section for a slot no sub-category covers", () => {
      expect(categoryHref({ category: "wearable", wearableCategory: "not_a_slot" })).toBe("/shop/items?category=wearable")
    })
  })

  describe("when the item is an emote", () => {
    it("should filter by the emote section", () => {
      expect(categoryHref({ category: "emote" })).toBe("/shop/items?category=emote")
    })
  })

  describe("when the item has no category at all", () => {
    it("should not link", () => {
      expect(categoryHref({ category: "names" })).toBeNull()
    })
  })

  describe("when the item is a smart wearable", () => {
    it("should filter by the smart flag, spelled the way the filter encodes a boolean", () => {
      expect(smartHref()).toBe("/shop/items?smart=true")
    })
  })
})
