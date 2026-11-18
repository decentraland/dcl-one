import { describe, expect, it } from 'vitest'
import { catalogTransport, COLLECTION, ITEM_ID, PRIMARY_WEARABLE, RESALE_WEARABLE } from '../../shop/discovery/fixtures'
import { createShopClient } from '@shop/api/shop'
import { fetchBuyListing } from '../shell/shop/purchase'

const OFF = (): boolean => false

/** A creator's sale of item 7 signed in MANA (1 MANA): the unified feed's `legacy` source, which the listings feed lacks. */
const LEGACY_PRIMARY = { ...PRIMARY_WEARABLE, source: 'legacy' as const, tradeId: 'trade-legacy-7', manaWei: '1000000000000000000', priceCredits: 3, listingCount: 1 }
/** The same item's USD-pegged sale on a creator discount: the row the server's per-item pick (native first) gives the card. */
const DISCOUNTED_NATIVE = { ...PRIMARY_WEARABLE, tradeId: 'trade-native-7', priceCredits: 10, compareAtCredits: 12, listingCount: 2 }

type Answer = (url: URL) => unknown

function shop(unified: Answer) {
  const transport = catalogTransport('ok', {
    '/v3/catalog/unified': unified,
    // The listings feed has no MANA-priced trade: a read of it would find nothing to buy.
    '/v3/catalog/shop': { data: [], total: 0 }
  })
  return { transport, client: createShopClient({ base: 'http://shop.test', mode: 'canonical', transport, signer: null }) }
}

/** The card's read (one grouped row per item) answers `grouped`; an ungrouped read answers its listings, newest first. */
const feed = (grouped: unknown[], listings: unknown[] = grouped): Answer => (url) => {
  const rows = url.searchParams.get('groupBy') === 'item' ? grouped : listings
  return { data: rows, total: rows.length }
}

const unifiedReads = (transport: ReturnType<typeof catalogTransport>) =>
  transport.calls.map((c) => new URL(c.url, 'http://shop.test')).filter((u) => u.pathname.endsWith('/v3/catalog/unified'))

describe("the in-world shell's BUY listing", () => {
  it("is the listing the card shows: the item's grouped row, not the newest of its listings", async () => {
    const { transport, client } = shop(feed([DISCOUNTED_NATIVE], [LEGACY_PRIMARY, DISCOUNTED_NATIVE]))
    expect(await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)).toMatchObject({ tradeId: 'trade-native-7' })
    const reads = unifiedReads(transport)
    expect(reads).toHaveLength(1)
    expect(Object.fromEntries(reads[0].searchParams)).toMatchObject({ groupBy: 'item', listingType: 'primary', first: '1', contractAddress: COLLECTION, itemId: ITEM_ID })
    expect(transport.calls.some((c) => new URL(c.url, 'http://shop.test').pathname.endsWith('/v3/catalog/shop'))).toBe(false)
  })

  it("finds a creator's MANA-priced (legacy) trade when that is the card's row", async () => {
    const { client } = shop(feed([LEGACY_PRIMARY]))
    expect(await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)).toMatchObject({ tradeId: 'trade-legacy-7', itemId: ITEM_ID, contractAddress: COLLECTION })
  })

  it("is a store mint, by acquisition and with no trade, when that is the card's row", async () => {
    const STORE_MINT = { ...PRIMARY_WEARABLE, source: 'legacy' as const, acquisition: 'store' as const, tradeId: null, manaWei: '4020000000000000000', listingCount: 1 }
    const { client } = shop(feed([STORE_MINT]))
    const listing = await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)
    expect(listing).toMatchObject({ acquisition: 'store', itemId: ITEM_ID, contractAddress: COLLECTION })
    expect(listing?.tradeId ?? null).toBeNull()
  })

  it('never takes a resale for the item, even when a primary read returns one', async () => {
    const { client } = shop(feed([RESALE_WEARABLE]))
    expect(await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)).toBeNull()
  })

  it('rejects when the feed cannot answer (no current MANA/USD rate), rather than saying the item is not on sale', async () => {
    const transport = catalogTransport('outage')
    const client = createShopClient({ base: 'http://shop.test', mode: 'canonical', transport, signer: null })
    await expect(fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)).rejects.toBeTruthy()
  })
})
