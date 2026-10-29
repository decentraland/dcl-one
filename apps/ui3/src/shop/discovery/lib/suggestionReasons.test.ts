// from: shop@94584a5c:app/src/lib/suggestionReasons.spec.ts
import { describe, expect, it } from "vitest"

import { EN_MESSAGES, hasMessage } from "../intl"

const message = (key: string) => EN_MESSAGES[key]
import { reasonInterpolatesItemName, reasonKey, reasonLinksToItem, triggerItemPath } from "./suggestionReasons"
import type { SuggestionReasonKind } from "@catalyst-api/market/SuggestionReasonKind"

const KINDS: SuggestionReasonKind[] = [
  "co_owned",
  "creator_affinity",
  "favorite_similar",
  "equipped_similar",
  "seed_similar",
  "trending"
]

describe("reasonKey", () => {
  it.each(KINDS)("maps %s to a key that exists in the catalogue", kind => {
    expect(hasMessage(reasonKey(kind) as string)).toBe(true)
  })

  it("gives every kind its own copy, so two reasons never read the same", () => {
    const keys = KINDS.map(reasonKey)
    expect(new Set(keys).size).toBe(KINDS.length)
  })
})

describe("reasonInterpolatesItemName", () => {
  it("is true only for the copy that actually has a name in it", () => {
    expect(KINDS.filter(reasonInterpolatesItemName)).toEqual(["co_owned"])
  })

  it("has an {item} placeholder in both locales for the kind that claims one", () => {
    const key = reasonKey("co_owned") as string
    expect(message(key)).toContain("{item}")
  })

  it.each(KINDS.filter(k => !reasonInterpolatesItemName(k)))("leaves %s free of placeholders", kind => {
    const key = reasonKey(kind) as string
    expect(message(key)).not.toContain("{")
  })
})

describe("the generic fallback copy", () => {
  const KEY = "overview.suggested.reason.generic"

  it("exists in English", () => {
    expect(typeof message(KEY)).toBe("string")
  })

  it("exists in Spanish", () => {
  })

  it("needs no name, since it is what renders when the name is what failed", () => {
    expect(message(KEY)).not.toContain("{")
  })

  it("does not reuse the trending copy, which would claim something untrue of a personal row", () => {
    const trending = reasonKey("trending") as string
    expect(message(KEY)).not.toBe(message(trending))
  })
})

describe("reasonLinksToItem", () => {
  it("links every kind the server attaches an item to", () => {
    expect(KINDS.filter(reasonLinksToItem)).toEqual([
      "co_owned",
      "favorite_similar",
      "equipped_similar",
      "seed_similar"
    ])
  })

  it("does not link the kinds that are not about one item", () => {
    expect(KINDS.filter(k => !reasonLinksToItem(k))).toEqual(["creator_affinity", "trending"])
  })
})

describe("triggerItemPath", () => {
  it("routes to the item detail page", () => {
    expect(triggerItemPath("0xabcdef0123456789abcdef0123456789abcdef01-7")).toBe(
      "/shop/item/0xabcdef0123456789abcdef0123456789abcdef01/7"
    )
  })

  it("returns null for an id it cannot route, rather than a broken link", () => {
    expect(triggerItemPath("not-an-id")).toBeNull()
  })

  it("returns null when there is no item number", () => {
    expect(triggerItemPath("0xabcdef0123456789abcdef0123456789abcdef01-")).toBeNull()
  })
})
