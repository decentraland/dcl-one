// from: shop@94584a5c:app/src/lib/storeStats.spec.ts (ported with its module)
import { describe, it, expect } from 'vitest'
import { bucketSales, buildStoreStats, TREND_POINTS } from './storeStats'
import type { SaleRow } from './sales'
import type { PublishableItem } from './builder'
import type { CollectionSaleState } from './collections'

const DAY = 86_400_000
const NOW = Date.UTC(2026, 0, 31, 23, 59, 59, 999)
const COLLECTION = '0xAbC0000000000000000000000000000000000001'

function row(over: Partial<SaleRow> & { itemId: string | null; daysAgo: number }): SaleRow {
  const { daysAgo, ...rest } = over
  return {
    id: `s-${over.itemId}-${daysAgo}`,
    contractAddress: COLLECTION,
    buyer: '0xbuyer',
    seller: '0xseller',
    price: '1000000000000000000',
    timestamp: NOW - daysAgo * DAY,
    type: 'mint',
    network: 'MATIC',
    tokenId: null,
    ...rest
  }
}

function item(over: Partial<PublishableItem> & { blockchainItemId: string }): PublishableItem {
  return {
    id: `b-${over.blockchainItemId}`,
    collectionId: 'col',
    collectionName: 'Galaxy Drip',
    contractAddress: COLLECTION,
    name: `Item ${over.blockchainItemId}`,
    category: 'hat',
    rarity: 'epic',
    thumbnail: 'thumb.png',
    type: 'wearable',
    isPublished: true,
    isApproved: true,
    totalSupply: 0,
    maxSupply: 10,
    remainingSupply: 10,
    minters: [],
    ...over
  }
}

const listed = (over: Partial<CollectionSaleState> = {}): CollectionSaleState => ({
  isOnSale: true,
  priceCredits: 30,
  ...over
})

function build(over: Partial<Parameters<typeof buildStoreStats>[0]> = {}) {
  return buildStoreStats({
    rows: [],
    total: 0,
    truncated: false,
    mints: 0,
    catalogue: [],
    saleState: {},
    days: 30,
    now: NOW,
    ...over
  })
}

describe('bucketSales', () => {
  it('spreads the window over a fixed number of points whatever its length', () => {
    const week = bucketSales([row({ itemId: '0', daysAgo: 0 })], 7, NOW)
    const year = bucketSales([row({ itemId: '0', daysAgo: 0 })], 365, NOW)
    expect(week).toHaveLength(TREND_POINTS)
    expect(year).toHaveLength(TREND_POINTS)
  })

  it('puts the newest sale in the last point and the oldest in the first', () => {
    const buckets = bucketSales([row({ itemId: '0', daysAgo: 0 }), row({ itemId: '1', daysAgo: 29.5 })], 30, NOW)
    expect(buckets[TREND_POINTS - 1]).toBe(1)
    expect(buckets[0]!).toBe(1)
    expect(buckets.reduce((n, b) => n + b, 0)).toBe(2)
  })

  it('drops a sale older than the window instead of piling it on the first point', () => {
    const buckets = bucketSales([row({ itemId: '0', daysAgo: 45 })], 30, NOW)
    expect(buckets).toEqual(new Array(TREND_POINTS).fill(0))
  })

  /** The window the feed was asked for includes its `from`, so a sale landing exactly on it is in range. */
  it('keeps a sale landing exactly on the start of the window', () => {
    const buckets = bucketSales([row({ itemId: '0', daysAgo: 30 })], 30, NOW)
    expect(buckets[0]!).toBe(1)
  })
})

describe('buildStoreStats', () => {
  it('counts a sale against the item that sold, whatever case the feed used for the address', () => {
    const stats = build({
      rows: [
        row({ itemId: '0', daysAgo: 1, contractAddress: COLLECTION.toUpperCase() }),
        row({ itemId: '0', daysAgo: 2 })
      ],
      total: 2,
      catalogue: [item({ blockchainItemId: '0' }), item({ blockchainItemId: '1' })]
    })
    const collection = stats.collections[0]!
    expect(collection.sold).toBe(2)
    expect(collection.items.find(i => i.itemId === '0')?.sold).toBe(2)
    expect(collection.items.find(i => i.itemId === '1')?.sold).toBe(0)
  })

  it('sums what each item earned, and only from its own first sales', () => {
    const stats = build({
      rows: [
        row({ itemId: '0', daysAgo: 1, price: '4000000000000000000' }),
        row({ itemId: '0', daysAgo: 2, price: '6000000000000000000' }),
        // A resale of the same item: it moves a copy that was already sold, so it is not the creator's
        // to count here — the row would otherwise inflate the item that never earned it.
        row({ itemId: '0', daysAgo: 3, price: '99000000000000000000', type: 'order' }),
        row({ itemId: '1', daysAgo: 1, price: '1000000000000000000' })
      ],
      total: 4,
      catalogue: [item({ blockchainItemId: '0' }), item({ blockchainItemId: '1' })]
    })
    const items = stats.collections[0]!.items
    expect(items.find(i => i.itemId === '0')?.earnedWei).toBe(10_000000000000000000n)
    expect(items.find(i => i.itemId === '1')?.earnedWei).toBe(1_000000000000000000n)
  })

  it('leaves an item that never sold at nothing earned rather than undefined', () => {
    const stats = build({ catalogue: [item({ blockchainItemId: '0' })] })
    expect(stats.collections[0]!.items[0]!.earnedWei).toBe(0n)
  })

  it('reads each item state from its supply and its listing', () => {
    const stats = build({
      catalogue: [
        item({ blockchainItemId: '0' }),
        item({ blockchainItemId: '1' }),
        item({ blockchainItemId: '2' }),
        item({ blockchainItemId: '3', remainingSupply: 0, totalSupply: 10 })
      ],
      saleState: {
        [`${COLLECTION}-0`]: listed(),
        [`${COLLECTION}-1`]: listed({ manaWei: '5000000000000000000' }),
        [`${COLLECTION}-2`]: listed({ isOnSale: false })
      }
    })
    expect({
      listed: stats.listed,
      classic: stats.classic,
      neverListed: stats.neverListed,
      soldOut: stats.soldOut
    }).toEqual({ listed: 1, classic: 1, neverListed: 1, soldOut: 1 })
    expect(stats.collections[0]!.items.map(i => i.state).sort()).toEqual([
      'classic',
      'discounted',
      'soldout',
      'unlisted'
    ])
  })

  /** A sold-out item reads as sold out even while its listing is still standing. */
  it('calls an item sold out before it calls it listed', () => {
    const stats = build({
      catalogue: [item({ blockchainItemId: '0', remainingSupply: 0, totalSupply: 10 })],
      saleState: { [`${COLLECTION}-0`]: listed() }
    })
    expect(stats.collections[0]!.items[0]!.state).toBe('soldout')
    expect(stats.listed).toBe(0)
  })

  it('splits the window by kind and sums what buyers paid', () => {
    const stats = build({
      rows: [
        row({ itemId: '0', daysAgo: 1, price: '2000000000000000000' }),
        row({ itemId: '0', daysAgo: 2, price: '3000000000000000000', type: 'order' })
      ],
      total: 2,
      mints: 1,
      catalogue: [item({ blockchainItemId: '0' })]
    })
    expect(stats.mints).toBe(1)
    expect(stats.resales).toBe(1)
    expect(stats.earningsWei).toBe(5_000_000_000_000_000_000n)
  })

  it('reports the feed total rather than the rows it fetched, and says which it summed', () => {
    const stats = build({ rows: [row({ itemId: '0', daysAgo: 1 })], total: 4000, truncated: true })
    expect(stats.sold).toBe(4000)
    expect(stats.partial).toBe(true)
    expect(stats.fetched).toBe(1)
  })

  it('opens each collection on its best seller and the page on its best collection', () => {
    const other = '0xdef0000000000000000000000000000000000002'
    const stats = build({
      rows: [row({ itemId: '1', daysAgo: 1 }), row({ itemId: '1', daysAgo: 2 })],
      total: 2,
      catalogue: [
        item({ blockchainItemId: '0', name: 'Aaa' }),
        item({ blockchainItemId: '1', name: 'Zzz' }),
        item({ blockchainItemId: '0', contractAddress: other, collectionName: 'Quiet drop' })
      ]
    })
    expect(stats.collections.map(c => c.name)).toEqual(['Galaxy Drip', 'Quiet drop'])
    expect(stats.collections[0]!.items.map(i => i.name)).toEqual(['Zzz', 'Aaa'])
  })

  it('takes the kind split from the feed"s own counts, not from the rows it fetched', () => {
    // The cap keeps the most recent rows, and a burst of first sales at the top would otherwise report a
    // store with hundreds of resales as having three.
    const rows = Array.from({ length: 3 }, (_, i) => row({ itemId: '0', daysAgo: i }))
    const stats = build({ rows, total: 2194, truncated: true, mints: 2160 })

    expect(stats.mints).toBe(2160)
    expect(stats.resales).toBe(34)
  })

  it('spans an all-time trend over the store"s own history, not a hardcoded year', () => {
    const stats = build({
      days: null,
      rows: [row({ itemId: '0', daysAgo: 900 }), row({ itemId: '0', daysAgo: 0 })],
      total: 2,
      catalogue: [item({ blockchainItemId: '0' })]
    })
    const trend = stats.collections[0]!.trend
    expect(trend[0]!).toBe(1)
    expect(trend[TREND_POINTS - 1]).toBe(1)
  })

  it('counts a resale in the totals but not against an item, because it is not part of the run', () => {
    const stats = build({
      rows: [row({ itemId: '0', daysAgo: 1, type: 'order' }), row({ itemId: '0', daysAgo: 1 })],
      total: 2,
      mints: 1,
      catalogue: [item({ blockchainItemId: '0' })]
    })

    expect(stats.sold).toBe(2)
    expect(stats.collections[0]!.items[0]!.sold).toBe(1)
    expect(stats.unattributed).toBe(1)
  })

  it('leaves a sale with no item out of the chart as well as out of the counts, so the two agree', () => {
    const stats = build({
      rows: [row({ itemId: null, daysAgo: 1 }), row({ itemId: '0', daysAgo: 1 })],
      total: 2,
      catalogue: [item({ blockchainItemId: '0' })]
    })
    const collection = stats.collections[0]!
    expect(collection.sold).toBe(1)
    expect(collection.trend.reduce((n, b) => n + b, 0)).toBe(1)
    expect(stats.unattributed).toBe(1)
  })

  it('counts a sale from a collection that is no longer published as unattributed', () => {
    const stats = build({
      rows: [row({ itemId: '0', daysAgo: 1, contractAddress: '0xgone' }), row({ itemId: '0', daysAgo: 1 })],
      total: 2,
      catalogue: [item({ blockchainItemId: '0' })]
    })
    expect(stats.collections).toHaveLength(1)
    expect(stats.unattributed).toBe(1)
  })

  it('gives an item no status at all when its collection could not be read, rather than calling it unlisted', () => {
    const stats = build({
      catalogue: [
        item({ blockchainItemId: '0' }),
        item({ blockchainItemId: '1', remainingSupply: 0, totalSupply: 10 })
      ],
      unreadable: new Set([COLLECTION.toLowerCase()])
    })
    expect(stats.collections[0]!.items.find(i => i.itemId === '0')?.state).toBe('unknown')
    expect(stats.neverListed).toBe(0)
    expect(stats.unknownCollections).toBe(1)
    // Supply is known whatever the listing feed said, so a sold-out item still reads as one.
    expect(stats.collections[0]!.items.find(i => i.itemId === '1')?.state).toBe('soldout')
  })

  it('draws the trend over what was actually read when the window is capped, not over the whole period', () => {
    // The cap keeps the most recent rows, so a busy store's fetched sales cover only the tail of the
    // period. Spanning the nominal 30 days would draw one spike against thirteen empty points.
    const rows = Array.from({ length: 6 }, (_, i) => row({ itemId: '0', daysAgo: i * 0.4 }))
    const stats = build({ rows, total: 5000, truncated: true, catalogue: [item({ blockchainItemId: '0' })] })

    expect(stats.trendDays).toBeLessThan(30)
    expect(stats.collections[0]!.trend.filter(n => n > 0).length).toBeGreaterThan(1)
  })

  it('keeps the whole period when the rows do cover it, so a quiet stretch reads as quiet', () => {
    const stats = build({
      rows: [row({ itemId: '0', daysAgo: 0 })],
      total: 1,
      catalogue: [item({ blockchainItemId: '0' })]
    })

    expect(stats.trendDays).toBe(30)
    expect(stats.collections[0]!.trend.filter(n => n > 0)).toHaveLength(1)
  })

  it('gives a collection with no sales a flat trend rather than none', () => {
    const stats = build({ catalogue: [item({ blockchainItemId: '0' })] })
    expect(stats.collections[0]!.trend).toEqual(new Array(TREND_POINTS).fill(0))
  })
})

/**
 * The server answers the window exactly, however big the store; the rows behind the breakdown are capped.
 * These pin which figure comes from which, because the failure they guard against is silent: a headline
 * that is right beside a breakdown that no longer admits it is only part of the story.
 */
describe('how much of a run is gone', () => {
  it('adds up claimed copies and the size of the run across the collection', () => {
    const stats = build({
      catalogue: [
        item({ blockchainItemId: '0', totalSupply: 30, remainingSupply: 70 }),
        item({ blockchainItemId: '1', totalSupply: 50, remainingSupply: 100 })
      ]
    })
    expect(stats.collections[0]!).toMatchObject({ claimed: 80, runTotal: 250 })
  })

  // Supply, not the window: moving the period selector does not change how much of a run has gone.
  it('is not touched by the window the page is showing', () => {
    const catalogue = [item({ blockchainItemId: '0', totalSupply: 30, remainingSupply: 70 })]
    expect(build({ catalogue, days: 7 }).collections[0]!.claimed).toBe(30)
    expect(build({ catalogue, days: null }).collections[0]!.claimed).toBe(30)
  })
})

describe('a collection with nothing left to sell', () => {
  const soldOut = (id: string) => item({ blockchainItemId: id, totalSupply: 10, remainingSupply: 0 })

  it('is marked when every item in it is gone, so a zero beside it reads as finished rather than failed', () => {
    const stats = build({ catalogue: [soldOut('0'), soldOut('1')] })
    expect(stats.collections[0]!.exhausted).toBe(true)
  })

  it('is not marked while one copy is still buyable', () => {
    const stats = build({ catalogue: [soldOut('0'), item({ blockchainItemId: '1' })] })
    expect(stats.collections[0]!.exhausted).toBe(false)
  })
})

describe('buildStoreStats with the server summary', () => {
  const summary = {
    total: 9_000,
    mints: 8_400,
    resales: 600,
    earnedWei: '4500000000000000000000',
    byCollection: [{ contractAddress: COLLECTION, sold: 8_400, earnedWei: '4200000000000000000000' }],
    byItem: [{ contractAddress: COLLECTION, itemId: '0', soldLifetime: 12_345 }],
    royalties: { resales: 77, volumeWei: '900000000000000000000' }
  }

  const withSummary = (over: Partial<Parameters<typeof buildStoreStats>[0]> = {}) =>
    build({
      summary,
      rows: [row({ itemId: '0', daysAgo: 1 }), row({ itemId: '0', daysAgo: 2 })],
      total: 2,
      mints: 2,
      catalogue: [item({ blockchainItemId: '0' })],
      ...over
    })

  it('takes every headline figure from the server rather than from the rows it happens to hold', () => {
    const stats = withSummary()

    expect(stats.sold).toBe(9_000)
    expect(stats.mints).toBe(8_400)
    expect(stats.resales).toBe(600)
    expect(stats.earningsWei).toBe(4_500_000_000_000_000_000_000n)
    expect(stats.royalties).toEqual({ resales: 77, volumeWei: 900_000_000_000_000_000_000n })
  })

  it("carries each item's lifetime sales, which no window of rows can answer", () => {
    expect(withSummary().collections[0]!.items[0]!.lifetimeSold).toBe(12_345)
  })

  it('stops calling the earnings a partial sum, because the server summed the whole window', () => {
    expect(withSummary({ truncated: true }).partial).toBe(false)
  })

  it('still says the breakdown is partial, because the items under it come from the capped rows', () => {
    const stats = withSummary({ truncated: true })

    expect(stats.breakdownPartial).toBe(true)
    // The header is the server's; the rows under it are what was fetched. The caveat above is what keeps
    // the difference from reading as a miscount.
    expect(stats.collections[0]!.sold).toBe(8_400)
    expect(stats.collections[0]!.items[0]!.sold).toBe(2)
  })

  it('measures what the list cannot account for against the headline, not against the capped rows', () => {
    // 9,000 sales in the window; 8,400 of them in the one collection this catalogue carries. The other 600
    // have no row to appear in, which is the whole reason the note exists. Comparing the capped rows
    // against the server's exact totals instead would report -6,398 and hide the note behind its own guard.
    const stats = withSummary({ truncated: true })

    expect(stats.unattributed).toBe(600)
  })

  it('falls back to comparing rows against rows when the server did not answer', () => {
    // A row on an item the catalogue does not carry: one of the two is attributable, the other is not.
    const stats = build({
      rows: [row({ itemId: '0', daysAgo: 1 }), row({ itemId: '404', daysAgo: 1 })],
      total: 2,
      mints: 2,
      catalogue: [item({ blockchainItemId: '0' })]
    })

    expect(stats.unattributed).toBe(1)
  })
})
