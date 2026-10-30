// from: shop@94584a5c:app/src/lib/storeMetrics.ts
import { weiOf, type SaleRow } from './sales'
import type { StoreCollection } from './storeStats'

const DAY_MS = 86_400_000

/**
 * A figure against the same figure one window earlier.
 *
 * `pct` is null when the earlier window held nothing: a rise from zero has no percentage, and rendering
 * one would either divide by zero or invent a baseline that was never there. The UI says "new" for that
 * case rather than a number.
 */
export type Delta = { current: number; previous: number; pct: number | null }

export function deltaOf(current: number, previous: number): Delta {
  return { current, previous, pct: previous === 0 ? null : ((current - previous) / previous) * 100 }
}

/**
 * The same, for MANA wei.
 *
 * The percentage is worked out in bigint and only then narrowed, so a large store's earnings do not lose
 * their last digits to a float on the way. The two raw figures are narrowed too and are past what a number
 * holds exactly, which is why nothing downstream divides them: {@link Delta.pct} already carries the
 * comparison, and a multiple can be read back off it.
 */
export function deltaOfWei(current: bigint, previous: bigint): Delta {
  const pct = previous === 0n ? null : Number(((current - previous) * 10_000n) / previous) / 100
  return { current: Number(current), previous: Number(previous), pct }
}

/**
 * Who bought from the STORE, and how many came back.
 *
 * First sales only. A creator is also the seller on any token they resell themselves, and those rows are a
 * different thing wearing the same shape: read against a real store, the top of this list was four LAND
 * sales from 2021 at up to 20,000 MANA, which swamped every person who had actually bought a wearable and
 * dated the table five years ago.
 *
 * A store with 200 sales across 190 buyers is a different business from one with 200 across 60, and
 * nothing on the page said which one you had. That question is about customers, so it is asked of the
 * sales that made them customers.
 */
export type Collectors = {
  /** Distinct buyers in the window. */
  total: number
  /** How many of them bought more than once. */
  repeat: number
  /** `repeat` as a percentage of `total`, 0 when nobody has bought yet. */
  repeatPct: number
  /** The most frequent buyers, best first. */
  top: { address: string; bought: number }[]
  /**
   * What share of the sales the single biggest buyer accounts for.
   *
   * The figure that reframes the rest of the page when it is high. A store reading "2,206 sold" where one
   * address took 2,202 of them does not have the business its headline suggests, and no other number here
   * would have said so.
   */
  topSharePct: number
}

export function collectorsOf(allRows: SaleRow[], topN = 3): Collectors {
  const rows = allRows.filter(isFirstSale)
  const byBuyer = new Map<string, number>()
  for (const row of rows) {
    const buyer = row.buyer.toLowerCase()
    byBuyer.set(buyer, (byBuyer.get(buyer) ?? 0) + 1)
  }
  const counts = [...byBuyer.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
  const repeat = counts.filter(([, bought]) => bought > 1).length
  return {
    total: counts.length,
    repeat,
    repeatPct: counts.length === 0 ? 0 : (repeat / counts.length) * 100,
    topSharePct: rows.length === 0 ? 0 : ((counts[0]?.[1] ?? 0) / rows.length) * 100,
    top: counts.slice(0, topN).map(([address, bought]) => ({ address, bought }))
  }
}

/**
 * The people behind the sales, best customer first. First sales only, for the reason {@link collectorsOf}
 * gives, and this table showed the cost of getting it wrong most plainly: a resale carries no item, so
 * every row read "0 items" while claiming five figures of spend.
 *
 * Counts of DISTINCT items and collections rather than of sales, because those are the two questions the
 * total cannot answer: someone who bought nine copies of one item is a fan of that item, and someone who
 * bought one thing from each of five collections is a fan of the creator. A store's figures read very
 * differently depending on which of the two it has.
 */
export type Buyer = {
  address: string
  /** Sales to them in the window. */
  bought: number
  /** How many different items, and from how many different collections. */
  items: number
  collections: number
  /** What they paid in total, in MANA wei. */
  spentWei: bigint
  /** When they last bought, in milliseconds. */
  lastAt: number
}

/** A copy bought from the creator's own stock, as opposed to one they resold from their own holdings. */
function isFirstSale(row: SaleRow): boolean {
  return row.type === 'mint' && row.itemId != null
}

export function topBuyers(allRows: SaleRow[], limit?: number): Buyer[] {
  const rows = allRows.filter(isFirstSale)
  const byBuyer = new Map<
    string,
    { bought: number; items: Set<string>; collections: Set<string>; spentWei: bigint; lastAt: number }
  >()
  for (const row of rows) {
    const address = row.buyer.toLowerCase()
    const contract = row.contractAddress.toLowerCase()
    const entry = byBuyer.get(address) ?? {
      bought: 0,
      items: new Set<string>(),
      collections: new Set<string>(),
      spentWei: 0n,
      lastAt: 0
    }
    entry.bought += 1
    entry.items.add(`${contract}-${row.itemId}`)
    entry.collections.add(contract)
    entry.spentWei += weiOf(row.price)
    if (row.timestamp > entry.lastAt) entry.lastAt = row.timestamp
    byBuyer.set(address, entry)
  }
  return [...byBuyer.entries()]
    .map(([address, e]) => ({
      address,
      bought: e.bought,
      items: e.items.size,
      collections: e.collections.size,
      spentWei: e.spentWei,
      lastAt: e.lastAt
    }))
    .sort((a, b) => (b.spentWei > a.spentWei ? 1 : b.spentWei < a.spentWei ? -1 : b.bought - a.bought))
    .slice(0, limit)
}

/**
 * How long since anything sold, in whole days.
 *
 * Null when the rows hold no sale, which does NOT mean the store has never sold: the rows only cover the
 * window that was asked for. The caller has that window and is the one that can phrase the difference.
 */
export function daysSinceLastSale(rows: SaleRow[], now: number): number | null {
  if (rows.length === 0) return null
  const last = rows.reduce((max, row) => (row.timestamp > max ? row.timestamp : max), 0)
  return Math.max(0, Math.floor((now - last) / DAY_MS))
}

/** One item of the store's top sellers, carrying the collection it came out of. */
export type BestSeller = {
  key: string
  contractAddress: string
  itemId: string
  name: string
  thumbnail: string
  rarity: string
  collectionName: string
  sold: number
  earnedWei: bigint
}

/**
 * The store's best sellers across every collection, best first.
 *
 * The collections table already orders each collection's own items, but nothing answered "what is selling
 * HERE" across the store: a creator with twenty collections would have to expand all of them and compare
 * by eye. Items that sold nothing in the window are left out rather than padding the list to five — a
 * table of zeroes is not a ranking.
 *
 * Ties break on earnings and then on name, so the order is stable: a store where six items each sold one
 * copy would otherwise reshuffle its top five on every render.
 */
export function bestSellers(collections: StoreCollection[], limit = 5): BestSeller[] {
  return collections
    .flatMap(collection =>
      collection.items
        .filter(item => item.sold > 0)
        .map(item => ({
          key: item.key,
          contractAddress: collection.contractAddress,
          itemId: item.itemId,
          name: item.name,
          thumbnail: item.thumbnail,
          rarity: item.rarity,
          collectionName: collection.name,
          sold: item.sold,
          earnedWei: item.earnedWei
        }))
    )
    .sort(
      (a, b) =>
        b.sold - a.sold ||
        (b.earnedWei > a.earnedWei ? 1 : b.earnedWei < a.earnedWei ? -1 : 0) ||
        a.name.localeCompare(b.name)
    )
    .slice(0, limit)
}
