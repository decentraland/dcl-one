import { describe, expect, it } from 'vitest'
import { catalogTransport, COLLECTION, ITEM_ID, PRIMARY_WEARABLE, RESALE_WEARABLE } from '../../shop/discovery/fixtures'
import { createShopClient } from '@shop/api/shop'
import { fetchBuyListing } from '../shell/shop/purchase'

const OFF = (): boolean => false

/** A creator's sale of item 7 signed in MANA (1 MANA): the unified feed's `legacy` source, which the listings feed lacks. */
const LEGACY_PRIMARY = { ...PRIMARY_WEARABLE, source: 'legacy' as const, tradeId: 'trade-legacy-7', manaWei: '1000000000000000000', priceCredits: 3 }

function shop(unified: (url: URL) => unknown) {
  const transport = catalogTransport('ok', {
    '/v3/catalog/unified': unified,
    // The listings feed has no MANA-priced trade: a read of it would find nothing to buy.
    '/v3/catalog/shop': { data: [], total: 0 }
  })
  return { transport, client: createShopClient({ base: 'http://shop.test', mode: 'canonical', transport, signer: null }) }
}

const asked = (transport: ReturnType<typeof catalogTransport>) => transport.calls.map((c) => new URL(c.url, 'http://shop.test'))

describe("the in-world shell's BUY listing", () => {
  it('finds a creator\'s MANA-priced trade the grid shows, reading the unified feed as the web Shop\'s item page does', async () => {
    const { transport, client } = shop((url) => ({ data: url.searchParams.get('listingType') === 'primary' ? [LEGACY_PRIMARY] : [], total: 1 }))
    const listing = await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)
    expect(listing).toMatchObject({ tradeId: 'trade-legacy-7', itemId: ITEM_ID, contractAddress: COLLECTION })
    const urls = asked(transport)
    expect(urls.some((u) => u.pathname.endsWith('/v3/catalog/shop'))).toBe(false)
    const unified = urls.filter((u) => u.pathname.endsWith('/v3/catalog/unified'))
    expect(unified.length).toBeGreaterThan(0)
    for (const u of unified) {
      expect(u.searchParams.get('listingType')).toBe('primary')
      expect(u.searchParams.get('contractAddress')).toBe(COLLECTION)
      expect(u.searchParams.get('itemId')).toBe(ITEM_ID)
    }
  })

  it('still finds a creator\'s USD-pegged trade', async () => {
    const { client } = shop(() => ({ data: [PRIMARY_WEARABLE], total: 1 }))
    expect(await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)).toMatchObject({ tradeId: 'trade-primary-7' })
  })

  it('never takes a resale for the item, even when a primary read returns one', async () => {
    const { client } = shop(() => ({ data: [RESALE_WEARABLE], total: 1 }))
    expect(await fetchBuyListing(COLLECTION, ITEM_ID, client, OFF)).toBeNull()
  })
})
