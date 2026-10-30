// from: shop@94584a5c:app/src/lib/storeMetrics.spec.ts (ported with its module)
import { describe, it, expect } from 'vitest'
import { bestSellers, collectorsOf, daysSinceLastSale, deltaOf, deltaOfWei, topBuyers } from './storeMetrics'
import type { SaleRow } from './sales'
import type { StoreCollection, StoreItem } from './storeStats'

const DAY = 86_400_000
const NOW = Date.UTC(2026, 0, 31, 12, 0, 0, 0)

function row(over: Partial<SaleRow> & { daysAgo: number }): SaleRow {
  const { daysAgo, ...rest } = over
  return {
    id: `s-${daysAgo}-${over.buyer ?? 'x'}`,
    itemId: '0',
    contractAddress: '0xabc',
    buyer: '0xbuyer1',
    seller: '0xseller',
    price: '1000000000000000000',
    timestamp: NOW - daysAgo * DAY,
    type: 'mint',
    network: 'MATIC',
    tokenId: null,
    ...rest
  }
}

describe('deltaOf', () => {
  it('reads a rise as a percentage of what came before', () => {
    expect(deltaOf(15, 9)).toEqual({ current: 15, previous: 9, pct: expect.closeTo(66.67, 1) })
  })

  it('reads a fall as a negative one', () => {
    expect(deltaOf(4, 8).pct).toBe(-50)
  })

  // A rise from nothing has no percentage. Inventing one would divide by zero, and picking a stand-in
  // number would tell a creator their first month grew by some amount over a month that did not happen.
  it('refuses a percentage when there was no baseline', () => {
    expect(deltaOf(7, 0).pct).toBeNull()
  })

  it('is flat at zero rather than absent when nothing happened either time', () => {
    expect(deltaOf(0, 0)).toEqual({ current: 0, previous: 0, pct: null })
  })
})

describe('deltaOfWei', () => {
  it('compares earnings without losing the large ones to floating point', () => {
    const current = 15_000_000_000_000_000_000_000n
    const previous = 12_000_000_000_000_000_000_000n
    expect(deltaOfWei(current, previous).pct).toBe(25)
  })

  it('refuses a percentage against nothing earned', () => {
    expect(deltaOfWei(5n, 0n).pct).toBeNull()
  })
})

describe('collectorsOf', () => {
  const rows = [
    row({ daysAgo: 1, buyer: '0xAAA' }),
    row({ daysAgo: 2, buyer: '0xaaa' }),
    row({ daysAgo: 3, buyer: '0xbbb' }),
    row({ daysAgo: 4, buyer: '0xccc' }),
    row({ daysAgo: 5, buyer: '0xccc' }),
    row({ daysAgo: 6, buyer: '0xccc' })
  ]

  it('counts people rather than sales, whatever case the address arrives in', () => {
    const collectors = collectorsOf(rows)
    expect(collectors.total).toBe(3)
    expect(collectors.repeat).toBe(2)
    expect(collectors.repeatPct).toBeCloseTo(66.67, 1)
  })

  it('ranks the most frequent buyers first', () => {
    expect(collectorsOf(rows, 2).top).toEqual([
      { address: '0xccc', bought: 3 },
      { address: '0xaaa', bought: 2 }
    ])
  })

  it('reports nobody rather than dividing by nobody', () => {
    expect(collectorsOf([])).toEqual({ total: 0, repeat: 0, repeatPct: 0, topSharePct: 0, top: [] })
  })

  it('counts customers of the store, not the people a creator resold a token to', () => {
    const mixed = [
      row({ daysAgo: 1, buyer: '0xcustomer' }),
      row({ daysAgo: 2, buyer: '0xflipper', itemId: null, type: 'order' })
    ]
    expect(collectorsOf(mixed).total).toBe(1)
  })

  /**
   * Taken from a real store: 2,206 sales in a month, of which one address took 2,202. Its headline read
   * like a thriving shop and its actual customer list was four people and a bot. This is the number that
   * says so, and it is why the count of buyers alone is not enough.
   */
  it('says how much of the store one buyer accounts for', () => {
    const lopsided = [
      ...Array.from({ length: 9 }, (_, i) => row({ daysAgo: i, buyer: '0xwhale' })),
      row({ daysAgo: 10, buyer: '0xsomeone' })
    ]
    expect(collectorsOf(lopsided).topSharePct).toBe(90)
  })
})

describe('topBuyers', () => {
  const rows = [
    // Nine copies of one item, from one collection: a fan of the item.
    ...Array.from({ length: 9 }, (_, i) => row({ daysAgo: i, buyer: '0xfan', itemId: '0' })),
    // One thing from each of three collections: a fan of the creator.
    row({ daysAgo: 1, buyer: '0xspread', itemId: '0', contractAddress: '0xaaa', price: '9000000000000000000' }),
    row({ daysAgo: 2, buyer: '0xspread', itemId: '1', contractAddress: '0xbbb', price: '9000000000000000000' }),
    row({ daysAgo: 3, buyer: '0xspread', itemId: '2', contractAddress: '0xccc', price: '9000000000000000000' })
  ]

  it('tells the two apart, which the count of sales alone cannot', () => {
    const [first, second] = topBuyers(rows)

    expect(second).toMatchObject({ address: '0xfan', bought: 9, items: 1, collections: 1 })
    expect(first).toMatchObject({ address: '0xspread', bought: 3, items: 3, collections: 3 })
  })

  it('ranks by what they spent, since that is the figure a creator is looking for', () => {
    // The spread buyer paid 27 against the fan's 9, so they lead despite a third of the sales.
    expect(topBuyers(rows)[0]!.spentWei).toBe(27_000_000_000_000_000_000n)
  })

  it('dates each one by their most recent purchase, not their first', () => {
    expect(topBuyers(rows).find(b => b.address === '0xfan')?.lastAt).toBe(NOW)
  })

  /**
   * Taken from a real store: its four biggest "buyers" were LAND resales from 2021, up to 20,000 MANA
   * each, which outranked every person who had bought a wearable and dated the table five years ago.
   * A resale also carries no item, so each of those rows read "0 items" beside five figures of spend.
   */
  it('leaves out the tokens a creator resold from their own holdings', () => {
    const withResales = [
      row({ daysAgo: 1, buyer: '0xcustomer', itemId: '0' }),
      row({ daysAgo: 2, buyer: '0xflipper', itemId: null, type: 'order', price: '20000000000000000000000' })
    ]
    const buyers = topBuyers(withResales)

    expect(buyers).toHaveLength(1)
    expect(buyers[0]!).toMatchObject({ address: '0xcustomer', items: 1 })
  })

  it('returns everyone when no limit is asked for, so the table can be paged', () => {
    const many = Array.from({ length: 12 }, (_, i) => row({ daysAgo: i, buyer: `0xb${i}`, itemId: '0' }))
    expect(topBuyers(many)).toHaveLength(12)
    expect(topBuyers(many, 5)).toHaveLength(5)
  })

  it('keeps only as many as asked for', () => {
    expect(topBuyers(rows, 1)).toHaveLength(1)
  })
})

describe('daysSinceLastSale', () => {
  it('measures from the most recent sale, not the oldest', () => {
    expect(daysSinceLastSale([row({ daysAgo: 12 }), row({ daysAgo: 3 })], NOW)).toBe(3)
  })

  it('says nothing rather than zero when the rows hold no sale', () => {
    expect(daysSinceLastSale([], NOW)).toBeNull()
  })
})

describe('bestSellers', () => {
  function item(name: string, sold: number, earned: bigint): StoreItem {
    return {
      key: `k-${name}`,
      itemId: name,
      name,
      thumbnail: '',
      rarity: 'epic',
      left: 10,
      minted: 20,
      sold,
      earnedWei: earned,
      lifetimeSold: null,
      priceCredits: 10,
      manaWei: null,
      createdAt: null,
      state: 'discounted'
    }
  }

  function collection(name: string, items: StoreItem[]): StoreCollection {
    return {
      contractAddress: `0x${name}`,
      collectionId: name,
      name,
      items,
      listed: items.length,
      classic: 0,
      soldOut: 0,
      sold: items.reduce((n, i) => n + i.sold, 0),
      earningsWei: 0n,
      createdAt: null,
      trend: [],
      claimed: 0,
      runTotal: 0,
      exhausted: false
    }
  }

  const collections = [
    collection('alpha', [item('a1', 3, 30n), item('a2', 9, 90n)]),
    collection('beta', [item('b1', 12, 120n), item('b2', 0, 0n)])
  ]

  it('ranks across collections, not inside one', () => {
    expect(bestSellers(collections).map(b => b.name)).toEqual(['b1', 'a2', 'a1'])
  })

  it('carries the collection each item came out of', () => {
    expect(bestSellers(collections)[0]!).toMatchObject({ name: 'b1', collectionName: 'beta', earnedWei: 120n })
  })

  it('leaves out what did not sell rather than padding the list with zeroes', () => {
    expect(bestSellers(collections).map(b => b.name)).not.toContain('b2')
  })

  it('breaks a tie on earnings, then on name, so the order does not reshuffle', () => {
    const tied = [collection('c', [item('zed', 5, 10n), item('amp', 5, 10n), item('rich', 5, 99n)])]
    expect(bestSellers(tied).map(b => b.name)).toEqual(['rich', 'amp', 'zed'])
  })

  it('keeps only as many as asked for', () => {
    expect(bestSellers(collections, 2).map(b => b.name)).toEqual(['b1', 'a2'])
  })
})
