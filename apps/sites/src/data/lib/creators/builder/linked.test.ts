import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest'
import { fetchAllLinkedCollectionItems, fetchThirdParty } from './linked'

vi.mock('~/lib/auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '~/lib/auth'
import { ADDRESS, okResponse } from '~/test/builderServer'

const signedFetchMock = signedFetch as Mock

beforeEach(() => {
  signedFetchMock.mockReset()
})

describe('fetchThirdParty', () => {
  it('answers the third party, with safe defaults for what a virtual one lacks', async () => {
    signedFetchMock.mockResolvedValue(okResponse({ id: 'urn:tp', name: 'Brand X', managers: [ADDRESS] }))
    await expect(fetchThirdParty(ADDRESS, 'urn:decentraland:amoy:collections-thirdparty:brand')).resolves.toMatchObject(
      { id: 'urn:tp', name: 'Brand X', managers: [ADDRESS], published: false, maxItems: '0', contracts: [] }
    )
  })
})

describe('fetchAllLinkedCollectionItems', () => {
  const remoteItem = (id: string) => ({
    id,
    name: id,
    description: '',
    thumbnail: 'thumbnail.png',
    eth_address: '',
    collection_id: 'c1',
    price: null,
    beneficiary: null,
    rarity: 'unique',
    is_published: false,
    is_approved: false,
    in_catalyst: false,
    type: 'wearable',
    data: { representations: [] },
    contents: {},
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z'
  })

  it('walks every page under the filter and keeps the filter on each request', async () => {
    signedFetchMock
      .mockResolvedValueOnce(
        okResponse({ results: [remoteItem('a'), remoteItem('b')], total: 3, page: 1, pages: 2, limit: 2 })
      )
      .mockResolvedValueOnce(okResponse({ results: [remoteItem('c')], total: 3, page: 2, pages: 2, limit: 2 }))
    const items = await fetchAllLinkedCollectionItems(ADDRESS, 'c1', { synced: false }, 2)
    expect(items.map(item => item.id)).toEqual(['a', 'b', 'c'])
    expect(signedFetchMock).toHaveBeenCalledTimes(2)
    expect(signedFetchMock.mock.calls[1][2]).toBe('/collections/c1/items?page=2&limit=2&synced=false')
  })
})
