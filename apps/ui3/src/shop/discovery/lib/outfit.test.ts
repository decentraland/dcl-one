// from: shop@94584a5c:app/src/lib/outfit.spec.ts
import { describe, it, expect, vi } from "vitest"
import {
  isWearable,
  slotOf,
  defaultWorn,
  toggleWorn,
  conflictingIds,
  wornUrns,
  outfitPreviewUrns,
  playingEmote
} from "./outfit"
import type { CatalogItem } from "./catalogItem"

// itemUrn (via wornUrns) reads config.chainId — pin it so URNs are deterministic (amoy).
vi.mock("~/config", () => ({ config: { chainId: 80002 } }))

function item(over: Partial<CatalogItem> & { id: string }): CatalogItem {
  return {
    name: over.id,
    creator: "",
    contractAddress: "0xc",
    itemId: "1",
    category: "wearable",
    rarity: "common",
    network: "MATIC",
    chainId: 80002,
    thumbnail: "",
    priceCredits: 1,
    gender: null,
    isSmart: false,
    ...over
  }
}

const hatA = item({ id: "a", wearableCategory: "hat", itemId: "10" })
const hatB = item({ id: "b", wearableCategory: "hat", itemId: "11" })
const top = item({ id: "c", wearableCategory: "upper_body", itemId: "12" })
const danceEmote = item({ id: "e", category: "emote", wearableCategory: "dance", itemId: "13" })

describe("isWearable", () => {
  it("is true for wearables and false for emotes", () => {
    expect(isWearable(hatA)).toBe(true)
    expect(isWearable(danceEmote)).toBe(false)
  })
})

describe("slotOf", () => {
  it("uses the wearable sub-category", () => {
    expect(slotOf(hatA)).toBe("hat")
    expect(slotOf(top)).toBe("upper_body")
  })
  it("falls back to a per-item slot when the category is unknown", () => {
    expect(slotOf(item({ id: "x", wearableCategory: undefined }))).toBe("unknown:x")
  })
  it("is null for an emote", () => {
    expect(slotOf(danceEmote)).toBeNull()
  })
})

describe("defaultWorn", () => {
  it("equips one wearable per slot (first wins) and skips emotes", () => {
    const worn = defaultWorn([hatA, hatB, top, danceEmote])
    expect([...worn].sort()).toEqual(["a", "c"]) // hatB dropped (slot taken), emote excluded
  })
})

describe("toggleWorn", () => {
  const all = [hatA, hatB, top]

  it("equips an item into a free slot", () => {
    const worn = toggleWorn(new Set(["a"]), top, all)
    expect([...worn].sort()).toEqual(["a", "c"])
  })

  it("swaps out the item already in the same slot", () => {
    const worn = toggleWorn(new Set(["a", "c"]), hatB, all)
    expect([...worn].sort()).toEqual(["b", "c"]) // hatA swapped out for hatB
  })

  it("unequips an already-worn item", () => {
    const worn = toggleWorn(new Set(["a", "c"]), hatA, all)
    expect([...worn].sort()).toEqual(["c"])
  })

  it("is a no-op for an emote", () => {
    const before = new Set(["a"])
    const after = toggleWorn(before, danceEmote, [...all, danceEmote])
    expect([...after]).toEqual(["a"])
  })
})

describe("conflictingIds", () => {
  it("flags every item sharing a slot with another", () => {
    expect([...conflictingIds([hatA, hatB, top])].sort()).toEqual(["a", "b"])
  })
  it("flags nothing when all slots are distinct", () => {
    expect(conflictingIds([hatA, top]).size).toBe(0)
  })
  it("ignores emotes", () => {
    expect(conflictingIds([top, danceEmote]).size).toBe(0)
  })
})

describe("wornUrns", () => {
  it("returns URNs for equipped wearables in cart order, skipping emotes", () => {
    const urns = wornUrns([hatA, top, danceEmote], new Set(["a", "c", "e"]))
    expect(urns).toEqual(["urn:decentraland:amoy:collections-v2:0xc:10", "urn:decentraland:amoy:collections-v2:0xc:12"])
  })
  it("skips items that are not equipped", () => {
    expect(wornUrns([hatA, top], new Set(["a"]))).toEqual(["urn:decentraland:amoy:collections-v2:0xc:10"])
  })
  it("skips wearables with no itemId (no equippable URN)", () => {
    const noItem = item({ id: "n", wearableCategory: "hat", itemId: null })
    expect(wornUrns([noItem], new Set(["n"]))).toEqual([])
  })
})

describe("outfitPreviewUrns", () => {
  const urn = (itemId: string) => `urn:decentraland:amoy:collections-v2:0xc:${itemId}`

  it("keeps the wearables in authored order when there is no emote", () => {
    expect(outfitPreviewUrns([hatA, top])).toEqual([urn("10"), urn("12")])
  })

  // Position, not membership: Unity's marketplace mode reads urns[0] and ignores the rest.
  it("puts the emote FIRST, ahead of every wearable", () => {
    expect(outfitPreviewUrns([hatA, top, danceEmote])).toEqual([urn("13"), urn("10"), urn("12")])
  })

  it("sends only the first emote, so both renderers play the same one", () => {
    const second = item({ id: "e2", category: "emote", wearableCategory: "fun", itemId: "14" })
    expect(outfitPreviewUrns([danceEmote, hatA, second])).toEqual([urn("13"), urn("10")])
  })

  it("skips items with no itemId", () => {
    const noItem = item({ id: "n", wearableCategory: "hat", itemId: null })
    expect(outfitPreviewUrns([noItem, top])).toEqual([urn("12")])
  })
})

describe("playingEmote", () => {
  it("is the first emote in authored order, or nothing", () => {
    const second = item({ id: "e2", category: "emote", itemId: "14" })
    expect(playingEmote([hatA, danceEmote, second])?.id).toBe("e")
    expect(playingEmote([hatA, top])).toBeUndefined()
  })
})
