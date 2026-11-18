import { afterEach, describe, expect, it, vi } from 'vitest'
import { maxUint256 } from 'viem'
import { fetchCollectionListings, fetchItemTradeId } from './listings'

const CONTRACT = '0x00000000000000000000000000000000000000cc'
const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function mockServer(items: unknown[], catalog: unknown[]) {
  fetchMock.mockImplementation((url: string) =>
    Promise.resolve(jsonResponse({ data: url.includes('/v1/items') ? items : catalog }))
  )
}

const creditsItem = { itemId: '0', price: '50000000000000000000', isOnSale: true, tradeId: 'trade-0' }
const manaItem = { itemId: '1', price: '5000000000000000000', isOnSale: true }
const freeItem = { itemId: '2', price: '0', isOnSale: true }
const notOnSale = { itemId: '3', price: '7000000000000000000', isOnSale: false }
const noPrice = { itemId: '4', price: maxUint256.toString(), isOnSale: true }
const malformed = { itemId: '5', price: '1.5', isOnSale: true }
const catalog = [
  { itemId: '0', priceCredits: 500, source: 'native' },
  { itemId: '1', priceCredits: 12, source: 'legacy' }
]

afterEach(() => fetchMock.mockReset())

describe('fetchCollectionListings', () => {
  it('prices credits listings from the shop catalog and everything else in MANA from the items endpoint', async () => {
    mockServer([creditsItem, manaItem, freeItem, notOnSale, noPrice, malformed], catalog)
    const listings = await fetchCollectionListings(CONTRACT)

    const urls = fetchMock.mock.calls.map(call => new URL(call[0] as string))
    expect(urls.map(url => url.origin + url.pathname).sort()).toEqual([
      'https://marketplace-api.decentraland.zone/v1/items',
      'https://marketplace-api.decentraland.zone/v3/catalog/unified'
    ])
    expect(urls.every(url => url.searchParams.get('contractAddress') === CONTRACT)).toBe(true)

    expect(listings.get('0')).toEqual({ itemId: '0', tradeId: 'trade-0', currency: 'credits', credits: 500 })
    expect(listings.get('1')).toEqual({
      itemId: '1',
      tradeId: undefined,
      currency: 'mana',
      manaWei: 5000000000000000000n
    })
    expect(listings.get('2')).toEqual({ itemId: '2', currency: 'mana', manaWei: 0n })
    expect(listings.has('3')).toBe(false)
    expect(listings.has('4')).toBe(false)
    expect(listings.has('5')).toBe(false)
  })

  it('says which marketplace each order was signed for, so a discount knows which listings it reaches', async () => {
    const onV3 = { ...creditsItem, tradeContractAddress: '0xE38EF22ABE871513555CBA89ADFE45AB4F548ADA' }
    mockServer([onV3, manaItem], catalog)
    const listings = await fetchCollectionListings(CONTRACT)

    expect(listings.get('0')?.tradeContract).toBe('0xe38ef22abe871513555cba89adfe45ab4f548ada')
    expect(listings.get('1')?.tradeContract).toBeUndefined()
  })

  it('fails when either request fails', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ error: 'nope' }, 500))
    await expect(fetchCollectionListings(CONTRACT)).rejects.toThrow(/500/)
  })
})

describe('fetchItemTradeId', () => {
  it('asks the unified catalog for the one item and answers its order id, or null without one', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        data: [
          { itemId: '3', tradeId: null },
          { itemId: '3', tradeId: 'trade-9' }
        ]
      })
    )
    await expect(fetchItemTradeId(CONTRACT, '3')).resolves.toBe('trade-9')
    const url = new URL(fetchMock.mock.calls[0][0] as string)
    expect(url.pathname).toBe('/v3/catalog/unified')
    expect(url.searchParams.get('itemId')).toBe('3')
    expect(url.searchParams.get('listingType')).toBe('primary')
    fetchMock.mockResolvedValueOnce(jsonResponse({ data: [] }))
    await expect(fetchItemTradeId(CONTRACT, '3')).resolves.toBeNull()
  })
})
