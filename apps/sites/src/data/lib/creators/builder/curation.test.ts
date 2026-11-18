import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest'
import {
  fetchCollectionCuration,
  fetchCommittee,
  fetchCurationCollections,
  fetchCurations,
  fetchItemCurations,
  pushCollectionCuration,
  updateCollectionCuration
} from './curation'
import { ValidationLimitError } from './review'

vi.mock('~/lib/auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '~/lib/auth'
import { ADDRESS, jsonResponse, okResponse, remoteCollection } from '~/test/builderServer'

const signedFetchMock = signedFetch as Mock

beforeEach(() => {
  signedFetchMock.mockReset()
})

const remoteCuration = {
  id: 'cu1',
  collection_id: 'a1b2',
  status: 'pending',
  assignee: '0xCURATOR',
  created_at: '2026-05-01T00:00:00Z',
  updated_at: '2026-05-02T00:00:00Z'
}

describe('fetchCollectionCuration', () => {
  it('answers the latest curation request of the collection', async () => {
    signedFetchMock.mockResolvedValue(okResponse(remoteCuration))
    await expect(fetchCollectionCuration(ADDRESS, 'a1b2')).resolves.toMatchObject({
      status: 'pending',
      assignee: '0xcurator'
    })
    expect(signedFetchMock.mock.calls[0][2]).toBe('/collections/a1b2/curation')
  })

  it('answers null for a collection that was never reviewed', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true }))
    await expect(fetchCollectionCuration(ADDRESS, 'a1b2')).resolves.toBeNull()
  })
})

describe('fetchCommittee', () => {
  it('answers the committee addresses, lowercased', async () => {
    signedFetchMock.mockResolvedValue(okResponse([{ id: '1', address: '0xABC' }]))
    await expect(fetchCommittee()).resolves.toEqual(['0xabc'])
    expect(signedFetchMock.mock.calls[0][2]).toBe('/committee')
  })
})

describe('fetchCurationCollections', () => {
  it('lists published standard collections for the committee', async () => {
    signedFetchMock.mockResolvedValue(
      okResponse({ results: [{ ...remoteCollection, is_published: true }], total: 1, page: 1, pages: 1, limit: 12 })
    )
    const result = await fetchCurationCollections(ADDRESS, {
      page: 1,
      search: '',
      statuses: [],
      assignees: [],
      reviewType: null,
      sort: 'REQUESTED_AT_DESC' as never,
      tag: null,
      type: 'all' as never
    })
    expect(result.results[0]).toMatchObject({ id: 'a1b2', isPublished: true })
    const path = signedFetchMock.mock.calls[0][2] as string
    expect(path.startsWith('/collections?is_published=true')).toBe(true)
    expect(path).not.toContain('type=')
  })
})

describe('curation requests', () => {
  it('lists every latest request', async () => {
    signedFetchMock.mockResolvedValue(okResponse([remoteCuration]))
    await expect(fetchCurations(ADDRESS)).resolves.toMatchObject([{ collectionId: 'a1b2' }])
  })

  it('opens a request, with an assignee when given one', async () => {
    signedFetchMock.mockResolvedValue(okResponse(remoteCuration))
    await pushCollectionCuration(ADDRESS, 'a1b2', '0xcurator')
    const [, , path, init] = signedFetchMock.mock.calls[0]
    expect(path).toBe('/collections/a1b2/curation')
    expect(init).toMatchObject({ method: 'POST', body: JSON.stringify({ curation: { assignee: '0xcurator' } }) })

    await pushCollectionCuration(ADDRESS, 'a1b2')
    expect(signedFetchMock.mock.calls[1][3].body).toBeUndefined()
  })

  it('patches the status or the assignee of the latest request', async () => {
    signedFetchMock.mockResolvedValue(okResponse({ ...remoteCuration, status: 'rejected' }))
    await expect(updateCollectionCuration(ADDRESS, 'a1b2', { status: 'rejected' })).resolves.toMatchObject({
      status: 'rejected'
    })
    expect(signedFetchMock.mock.calls[0][3]).toMatchObject({
      method: 'PATCH',
      body: JSON.stringify({ curation: { status: 'rejected' } })
    })
  })
})

describe('fetchItemCurations', () => {
  it("answers each item's latest curation status by item id", async () => {
    signedFetchMock.mockResolvedValue(
      okResponse([
        { id: 'ic1', item_id: 'i1', status: 'approved' },
        { id: 'ic2', item_id: 'i2', status: 'pending' }
      ])
    )
    const curations = await fetchItemCurations(ADDRESS, 'a1b2')
    expect(curations.get('i1')).toMatchObject({ itemId: 'i1', status: 'approved' })
    expect(curations.get('i2')).toMatchObject({ itemId: 'i2', status: 'pending' })
    expect(curations.has('i3')).toBe(false)
  })

  it('answers no curations for a collection none of whose items was submitted yet', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: false, error: 'Unpublished collection' }, false, 409))
    await expect(fetchItemCurations(ADDRESS, 'a1b2')).resolves.toEqual(new Map())
  })

  it('still fails on any other error', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: false, error: 'Unauthorized' }, false, 401))
    await expect(fetchItemCurations(ADDRESS, 'a1b2')).rejects.toThrow('Unauthorized')
  })
})

describe('review requests under the automated review', () => {
  it("turns the daily limit on a creator's request into the daily-limit error", async () => {
    signedFetchMock.mockResolvedValue(
      jsonResponse({ ok: false, error: 'Too many attempts', data: { retryAt: '2026-09-30T00:00:00Z' } }, false, 429)
    )
    const error = await pushCollectionCuration(ADDRESS, 'a1b2').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(ValidationLimitError)
  })

  it('sends the rejection reasons and message with the status', async () => {
    signedFetchMock.mockResolvedValue(okResponse({ ...remoteCuration, status: 'rejected' }))
    await updateCollectionCuration(ADDRESS, 'a1b2', {
      status: 'rejected',
      rejectionReasons: ['clipping'],
      rejectionMessage: 'Clips through the torso'
    })
    expect(signedFetchMock.mock.calls[0][3]).toMatchObject({
      body: JSON.stringify({
        curation: { status: 'rejected', rejectionReasons: ['clipping'], rejectionMessage: 'Clips through the torso' }
      })
    })
  })
})
