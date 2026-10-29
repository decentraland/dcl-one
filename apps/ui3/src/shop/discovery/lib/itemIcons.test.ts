// from: shop@94584a5c:app/src/lib/itemIcons.spec.ts
import { describe, it, expect } from "vitest"
import { categoryIcon, genderIcon } from "./itemIcons"

const item = (over: Partial<Parameters<typeof categoryIcon>[0]> = {}) => ({
  category: "wearable",
  wearableCategory: undefined,
  ...over
})

describe("categoryIcon", () => {
  it("maps on-chain wearable categories to their sprite icon", () => {
    expect(categoryIcon(item({ wearableCategory: "hat" }))).toBe("cat-head")
    expect(categoryIcon(item({ wearableCategory: "upper_body" }))).toBe("cat-upper")
    expect(categoryIcon(item({ wearableCategory: "hands_wear" }))).toBe("cat-handwear")
    expect(categoryIcon(item({ wearableCategory: "lower_body" }))).toBe("cat-lower")
    expect(categoryIcon(item({ wearableCategory: "feet" }))).toBe("cat-feet")
    expect(categoryIcon(item({ wearableCategory: "eyewear" }))).toBe("cat-accessories")
    expect(categoryIcon(item({ wearableCategory: "skin" }))).toBe("cat-skins")
  })

  it("maps emote sub-categories, and falls back to the generic emote glyph", () => {
    expect(categoryIcon(item({ category: "emote", wearableCategory: "dance" }))).toBe("emote-dance")
    expect(categoryIcon(item({ category: "emote", wearableCategory: undefined }))).toBe("emote-misc")
    expect(categoryIcon(item({ category: "emote", wearableCategory: "not-a-real-category" }))).toBe("emote-misc")
  })

  it("is case-insensitive on the on-chain category", () => {
    expect(categoryIcon(item({ wearableCategory: "UPPER_BODY" }))).toBe("cat-upper")
  })

  it("returns null for an unknown wearable category (no chip rather than a wrong icon)", () => {
    expect(categoryIcon(item({ wearableCategory: "mystery" }))).toBeNull()
    expect(categoryIcon(item({ wearableCategory: undefined }))).toBeNull()
  })
})

describe("genderIcon", () => {
  it("maps each gender to its icon", () => {
    expect(genderIcon("male")).toBe("gender-male")
    expect(genderIcon("female")).toBe("gender-female")
    expect(genderIcon("unisex")).toBe("gender-unisex")
  })

  it("returns null when gender is absent", () => {
    expect(genderIcon(null)).toBeNull()
  })
})
