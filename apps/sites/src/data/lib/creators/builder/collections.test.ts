import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest'
import { deleteCollection, fetchCollectionItemPreviews, fetchCollections } from './collections'

vi.mock('~/lib/auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '~/lib/auth'
import { ADDRESS, jsonResponse, okResponse, remoteCollection } from '~/test/builderServer'

const signedFetchMock = signedFetch as Mock

beforeEach(() => {
  signedFetchMock.mockReset()
})

describe('fetchCollections', () => {
  it('requests the paginated creator collections and maps the results', async () => {
    signedFetchMock.mockResolvedValue(
      okResponse({ results: [remoteCollection], total: 1, page: 1, pages: 1, limit: 20 })
    )

    const result = await fetchCollections(ADDRESS, { page: 1, q: 'hat' })

    const [address, baseUrl, path] = signedFetchMock.mock.calls[0] as [string, string, string]
    expect(address).toBe(ADDRESS)
    expect(baseUrl).toBe('https://builder-api.decentraland.zone/v1')
    // page+limit are ALWAYS sent — without both, builder-server answers a bare array.
    expect(path).toBe(`/${ADDRESS}/collections?q=hat&page=1&limit=20`)
    expect(result.total).toBe(1)
    expect(result.results[0]).toMatchObject({ id: 'a1b2', name: 'Pirate Hats', itemCount: 3 })
  })
})

describe('fetchCollectionItemPreviews', () => {
  it('fetches the first items and resolves their content-addressed thumbnail URLs', async () => {
    signedFetchMock.mockResolvedValue(
      okResponse({
        results: [{ id: 'i1', name: 'Hat', thumbnail: 'thumbnail.png', contents: { 'thumbnail.png': 'Qmhash' } }]
      })
    )

    const previews = await fetchCollectionItemPreviews(ADDRESS, 'a1b2')

    const [, , path] = signedFetchMock.mock.calls[0] as [string, string, string]
    expect(path).toBe('/collections/a1b2/items?page=1&limit=4')
    expect(previews).toEqual([
      { id: 'i1', name: 'Hat', thumbnailUrl: 'https://builder-api.decentraland.zone/v1/storage/contents/Qmhash' }
    ])
  })

  it('drops items whose thumbnail hash is missing from contents', async () => {
    signedFetchMock.mockResolvedValue(
      okResponse({
        results: [
          { id: 'i1', name: 'Hat', thumbnail: 'thumbnail.png', contents: { 'thumbnail.png': 'Qmhash' } },
          { id: 'i2', name: 'Broken', thumbnail: 'thumbnail.png', contents: {} }
        ]
      })
    )

    const previews = await fetchCollectionItemPreviews(ADDRESS, 'a1b2')

    expect(previews.map(p => p.id)).toEqual(['i1'])
  })
})

describe('deleteCollection', () => {
  beforeEach(() => signedFetchMock.mockReset())

  it('sends a signed DELETE for the collection', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true, data: true }))
    await deleteCollection(ADDRESS, 'a1b2')
    expect(signedFetchMock).toHaveBeenCalledWith(
      ADDRESS,
      expect.any(String),
      '/collections/a1b2',
      expect.objectContaining({ method: 'DELETE' })
    )
  })

  it('surfaces the server status when the collection can no longer be deleted', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: false, error: 'already published' }, false, 409))
    await expect(deleteCollection(ADDRESS, 'a1b2')).rejects.toMatchObject({ status: 409 })
  })
})
