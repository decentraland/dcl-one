// from: shop@94584a5c:app/src/lib/topCreators.spec.ts
import { describe, it, expect } from "vitest"
import type { CreatorProfile } from "./topCreators"
import { selectTopCreators } from "./topCreators"

// A ranking row. The tuple is [address, window sales]; the figures the card shows are derived from it so
// a test can tell which number reached the card without spelling out four every time.
// A ranking row. The tuple is [address, window sales]; the figures the card shows are derived from it so
// a test can tell which number reached the card without spelling out four every time. `items` can be
// overridden per address to drive the catalogue floor.
const ranked = (...rows: [string, number][]) =>
  rows.map(([id, sales]) => ({ id, sales, totalSales: sales * 100, collections: 3, items: 30 }))

function profiles(entries: Record<string, Partial<CreatorProfile>>): Map<string, CreatorProfile> {
  return new Map(Object.entries(entries).map(([address, profile]) => [address.toLowerCase(), profile as CreatorProfile]))
}

describe("selectTopCreators", () => {
  it("should keep the ranking order and carry each creator sale count onto the card", () => {
    const selected = selectTopCreators(
      ranked(["0xa", 62], ["0xb", 34]),
      profiles({
        "0xa": { name: "byPolygonalMind", hasClaimedName: true },
        "0xb": { name: "METATIGER", hasClaimedName: true }
      }),
      8
    )

    expect(selected.map(creator => [creator.name, creator.totalSales, creator.collections, creator.items])).toEqual([
      ["byPolygonalMind", 6200, 3, 30],
      ["METATIGER", 3400, 3, 30]
    ])
  })

  /**
   * The one that put a wallet called `test` third on the production row. An unclaimed name renders as
   * `test#488a`, which is not a creator anyone can be introduced to — and buying a name is the cheapest
   * signal available that there is a real storefront behind the sales.
   */
  it("should drop a creator whose name is not claimed, however well they sell", () => {
    const selected = selectTopCreators(
      ranked(["0xtest", 999], ["0xreal", 1]),
      profiles({ "0xtest": { name: "test", hasClaimedName: false }, "0xreal": { name: "Saus", hasClaimedName: true } }),
      8
    )

    expect(selected.map(creator => creator.name)).toEqual(["Saus"])
  })

  it("should drop a creator with no profile at all", () => {
    const selected = selectTopCreators(
      ranked(["0xghost", 27], ["0xreal", 1]),
      profiles({ "0xreal": { name: "Saus", hasClaimedName: true } }),
      8
    )

    expect(selected.map(creator => creator.name)).toEqual(["Saus"])
  })

  it("should drop a creator whose claimed name is blank", () => {
    const selected = selectTopCreators(
      ranked(["0xblank", 40], ["0xreal", 1]),
      profiles({ "0xblank": { name: "   ", hasClaimedName: true }, "0xreal": { name: "Saus", hasClaimedName: true } }),
      8
    )

    expect(selected.map(creator => creator.name)).toEqual(["Saus"])
  })

  // A creator can hold several wallets, and two cards under one name read as a bug rather than as two
  // people. The higher-ranked wallet is the one that stays.
  it("should show a name once, keeping the better-selling wallet", () => {
    const selected = selectTopCreators(
      ranked(["0xmain", 62], ["0xalt", 20], ["0xother", 5]),
      profiles({
        "0xmain": { name: "Doki3D", hasClaimedName: true },
        "0xalt": { name: "doki3d", hasClaimedName: true },
        "0xother": { name: "Cansy", hasClaimedName: true }
      }),
      8
    )

    expect(selected.map(creator => [creator.address, creator.totalSales])).toEqual([
      ["0xmain", 6200],
      ["0xother", 500]
    ])
  })

  // The rail asks for far more candidates than it can show, precisely so the filters have something to
  // eat into; the surplus must not reach the row.
  it("should stop at the limit", () => {
    const rows = ranked(["0xa", 5], ["0xb", 4], ["0xc", 3])
    const all = profiles({
      "0xa": { name: "A", hasClaimedName: true },
      "0xb": { name: "B", hasClaimedName: true },
      "0xc": { name: "C", hasClaimedName: true }
    })

    expect(selectTopCreators(rows, all, 2).map(creator => creator.name)).toEqual(["A", "B"])
  })

  // The ranking's addresses come from the indexer and the profiles from the Catalyst, which echoes back
  // its own casing — matching them literally would drop every creator on the row.
  it("should match an address regardless of how either side cased it", () => {
    const selected = selectTopCreators(
      ranked(["0xAbC", 62]),
      profiles({ "0xabc": { name: "Canessa", hasClaimedName: true } }),
      8
    )

    expect(selected.map(creator => creator.name)).toEqual(["Canessa"])
  })

  it("should carry the face snapshot when the profile has one", () => {
    const selected = selectTopCreators(
      ranked(["0xa", 1]),
      profiles({ "0xa": { name: "Saus", hasClaimedName: true, face: "face.png" } }),
      8
    )

    expect(selected[0]!.face).toBe("face.png")
  })
})
