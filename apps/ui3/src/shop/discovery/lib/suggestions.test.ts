// from: shop@94584a5c:app/src/lib/suggestions.spec.ts
import { describe, expect, it } from "vitest"

import type { CatalogItem } from "./catalogItem"
import { mergeSuggestions, SUGGESTIONS_TARGET } from "./suggestions"

/**
 * The PDP rail's fill order.
 *
 * Most collections hold two or three items, so the interesting cases are all about PADDING: what gets
 * appended, in what order, and what must never appear twice. The creator tier is the same feed as the
 * collection tier filtered by creator, so it ALWAYS re-serves the collection's items — deduping across
 * tiers is the whole reason this is a function and not a concat.
 */

const ANCHOR_CONTRACT = "0xANCHOR"

function item(over: Partial<CatalogItem> & { id: string }): CatalogItem {
  return {
    name: "Item",
    creator: "0xcreator",
    contractAddress: ANCHOR_CONTRACT,
    itemId: over.id,
    category: "wearable",
    rarity: "rare",
    network: "MATIC",
    chainId: 80002,
    thumbnail: "",
    priceCredits: 10,
    gender: "unisex",
    isSmart: false,
    ...over
  }
}

const anchor = { id: "a", contractAddress: ANCHOR_CONTRACT, itemId: "a" }

// n items from one contract, ids prefixed so a tier's contribution is identifiable in the output.
function items(prefix: string, n: number, over: Partial<CatalogItem> = {}): CatalogItem[] {
  return Array.from({ length: n }, (_, i) => item({ id: `${prefix}${i}`, itemId: `${prefix}${i}`, ...over }))
}

const ids = (list: CatalogItem[]) => list.map(i => i.id)

describe("mergeSuggestions", () => {
  it("should drop the item being viewed, whichever tier serves it", () => {
    const merged = mergeSuggestions(
      {
        collection: [item({ id: "a", itemId: "a" }), item({ id: "b", itemId: "b" })],
        creator: [item({ id: "a2", itemId: "a" })]
      },
      anchor
    )

    expect(ids(merged.items)).toEqual(["b"])
  })

  it("should match the anchor by item within its own contract, not by item id alone", () => {
    // A different collection can reuse item id "a" — that is a different item and belongs on the rail.
    const merged = mergeSuggestions(
      { collection: [item({ id: "other-a", contractAddress: "0xOTHER", itemId: "a" })] },
      anchor
    )

    expect(ids(merged.items)).toEqual(["other-a"])
  })

  it("should recognise the anchor across feeds that id the same item differently", () => {
    // The unified feeds put the tradeId in `id`; the catalog feed puts contract-itemId there. Only
    // (contract, itemId) identifies the item in both.
    const merged = mergeSuggestions({ collection: [item({ id: "trade-99", itemId: "a" })] }, anchor)

    expect(merged.items).toEqual([])
  })

  it("should pad a short collection with the creator tier up to the target", () => {
    const merged = mergeSuggestions({ collection: items("c", 3), creator: items("k", 40) }, anchor)

    expect(merged.items).toHaveLength(SUGGESTIONS_TARGET)
    expect(ids(merged.items).slice(0, 3)).toEqual(["c0", "c1", "c2"])
    expect(merged.isCollectionOnly).toBe(false)
  })

  it("should not re-list a collection item that the creator tier serves again", () => {
    const merged = mergeSuggestions(
      { collection: items("c", 2), creator: [...items("c", 2), ...items("k", 5)] },
      anchor
    )

    expect(ids(merged.items)).toEqual(["c0", "c1", "k0", "k1", "k2", "k3", "k4"])
  })

  it("should fall through to the related tier when the creator tier still leaves the rail short", () => {
    const merged = mergeSuggestions(
      { collection: items("c", 2), creator: items("k", 3), related: items("r", 40) },
      anchor
    )

    expect(merged.items).toHaveLength(SUGGESTIONS_TARGET)
    expect(ids(merged.items).slice(0, 5)).toEqual(["c0", "c1", "k0", "k1", "k2"])
    expect(ids(merged.items).at(-1)).toBe("r9")
  })

  it("should dedupe the related tier against everything already on the rail", () => {
    const merged = mergeSuggestions(
      {
        collection: items("c", 1),
        creator: items("k", 1),
        related: [...items("c", 1), ...items("k", 1), ...items("r", 2)]
      },
      anchor
    )

    expect(ids(merged.items)).toEqual(["c0", "k0", "r0", "r1"])
  })

  it("should show a large collection in full rather than truncating it to the target", () => {
    const merged = mergeSuggestions({ collection: items("c", 19), creator: items("k", 10) }, anchor)

    expect(merged.items).toHaveLength(19)
    expect(merged.isCollectionOnly).toBe(true)
  })

  it("should report a rail as collection-only when the padding tiers had nothing to add", () => {
    // 14 siblings is under the target, but nothing was appended — the collection heading and its
    // "View all" are still accurate, so the flag must not key off the target.
    const merged = mergeSuggestions({ collection: items("c", 14), creator: items("c", 14), related: [] }, anchor)

    expect(merged.items).toHaveLength(14)
    expect(merged.isCollectionOnly).toBe(true)
  })

  it("should stay empty — never a bare rail — when no tier has anything", () => {
    const merged = mergeSuggestions({ collection: [], creator: [], related: [] }, anchor)

    expect(merged.items).toEqual([])
    expect(merged.isCollectionOnly).toBe(false)
  })

  it("should treat a token listing as the anchor when the page is showing that token", () => {
    const merged = mergeSuggestions(
      {
        collection: [item({ id: "l1", itemId: "z", tokenId: "5013" }), item({ id: "l2", itemId: "z", tokenId: "77" })]
      },
      { contractAddress: ANCHOR_CONTRACT, tokenId: "5013" }
    )

    expect(ids(merged.items)).toEqual(["l2"])
  })

  it("should honour an explicit target", () => {
    const merged = mergeSuggestions({ collection: items("c", 1), creator: items("k", 40) }, anchor, 4)

    expect(ids(merged.items)).toEqual(["c0", "k0", "k1", "k2"])
  })
})
