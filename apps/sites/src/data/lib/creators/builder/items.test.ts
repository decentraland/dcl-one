import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest'
import { saveItem } from './items'

vi.mock('~/lib/auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '~/lib/auth'
import { ADDRESS, jsonResponse, okResponse } from '~/test/builderServer'
import { ItemType, type Item } from '~/lib/items'

const signedFetchMock = signedFetch as Mock

beforeEach(() => {
  signedFetchMock.mockReset()
})

describe('saveItem', () => {
  const remoteItem = {
    id: 'item-1',
    name: 'Hat',
    description: '',
    thumbnail: 'thumbnail.png',
    eth_address: ADDRESS,
    collection_id: 'a1b2',
    price: null,
    beneficiary: null,
    rarity: 'epic',
    is_published: false,
    is_approved: false,
    in_catalyst: false,
    type: 'wearable',
    data: { representations: [] },
    contents: { 'male/model.glb': 'QmModel', 'thumbnail.png': 'QmThumb' },
    created_at: '2026-03-01T10:00:00Z',
    updated_at: '2026-03-01T10:00:00Z'
  }

  const item: Item = {
    id: 'item-1',
    name: 'Hat',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: ADDRESS,
    collectionId: 'a1b2',
    rarity: 'epic',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { representations: [] },
    contents: { 'male/model.glb': 'QmModel', 'thumbnail.png': 'QmThumb' },
    createdAt: 1,
    updatedAt: 1
  }

  it('PUTs the item then POSTs its files keyed by content hash', async () => {
    signedFetchMock.mockResolvedValueOnce(okResponse(remoteItem)).mockResolvedValueOnce(jsonResponse({ ok: true }))

    const blobs = { 'male/model.glb': new Blob(['model']), 'thumbnail.png': new Blob(['thumb']) }
    const saved = await saveItem(ADDRESS, item, blobs)

    expect(saved.id).toBe('item-1')
    const [, , putPath, putInit] = signedFetchMock.mock.calls[0] as [string, string, string, RequestInit]
    expect(putPath).toBe('/items/item-1')
    expect(putInit.method).toBe('PUT')
    const putBody = JSON.parse(putInit.body as string) as { item: { eth_address: string; is_published: boolean } }
    expect(putBody.item.eth_address).toBe(ADDRESS)
    expect(putBody.item.is_published).toBe(false)

    const [, , postPath, postInit] = signedFetchMock.mock.calls[1] as [string, string, string, RequestInit]
    expect(postPath).toBe('/items/item-1/files')
    expect(postInit.body).toBeInstanceOf(FormData)
    const formKeys = Array.from((postInit.body as FormData).keys()).sort()
    expect(formKeys).toEqual(['QmModel', 'QmThumb'])
  })

  it('POSTs a smart wearable video to the videos endpoint keyed by path', async () => {
    signedFetchMock
      .mockResolvedValueOnce(okResponse(remoteItem))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))

    const smartItem = { ...item, contents: { ...item.contents, 'video.mp4': 'QmVideo' } }
    await saveItem(ADDRESS, smartItem, { 'male/model.glb': new Blob(['model']), 'video.mp4': new Blob(['video']) })

    const [, , filesPath, filesInit] = signedFetchMock.mock.calls[1] as [string, string, string, RequestInit]
    expect(filesPath).toBe('/items/item-1/files')
    expect(Array.from((filesInit.body as FormData).keys())).toEqual(['QmModel'])
    const [, , videosPath, videosInit] = signedFetchMock.mock.calls[2] as [string, string, string, RequestInit]
    expect(videosPath).toBe('/items/item-1/videos')
    expect(Array.from((videosInit.body as FormData).keys())).toEqual(['video.mp4'])
  })

  it('drops the video reference again when the video upload fails', async () => {
    signedFetchMock
      .mockResolvedValueOnce(okResponse(remoteItem))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: false, error: 'too large' }, false, 413))
      .mockResolvedValueOnce(okResponse(remoteItem))

    const smartItem = { ...item, video: 'QmVideo', contents: { ...item.contents, 'video.mp4': 'QmVideo' } }
    await expect(
      saveItem(ADDRESS, smartItem, { 'male/model.glb': new Blob(['model']), 'video.mp4': new Blob(['video']) })
    ).rejects.toMatchObject({ status: 413 })

    const [, , path, init] = signedFetchMock.mock.calls[3] as [string, string, string, RequestInit]
    expect(path).toBe('/items/item-1')
    const body = JSON.parse(init.body as string) as { item: { video?: string; contents: Record<string, string> } }
    expect(body.item.video).toBeFalsy()
    expect(body.item.contents['video.mp4']).toBeUndefined()
  })

  it('points an approved item back at its approved video when the new upload fails', async () => {
    signedFetchMock
      .mockResolvedValueOnce(okResponse(remoteItem))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: false, error: 'too large' }, false, 413))
      .mockResolvedValueOnce(okResponse(remoteItem))

    // `video` is the curation-approved hash; `contents` already carries the new one the PUT stored.
    const approved = {
      ...item,
      isPublished: true,
      isApproved: true,
      video: 'QmApproved',
      contents: { ...item.contents, 'video.mp4': 'QmNewVideo' }
    }
    await expect(
      saveItem(ADDRESS, approved, { 'male/model.glb': new Blob(['model']), 'video.mp4': new Blob(['video']) })
    ).rejects.toMatchObject({ status: 413 })

    const [, , path, init] = signedFetchMock.mock.calls[3] as [string, string, string, RequestInit]
    expect(path).toBe('/items/item-1')
    const body = JSON.parse(init.body as string) as { item: { video?: string; contents: Record<string, string> } }
    expect(body.item.video).toBe('QmApproved')
    expect(body.item.contents['video.mp4']).toBe('QmApproved')
  })

  it('restores the video an unapproved item had when the caller passes it and the new upload fails', async () => {
    signedFetchMock
      .mockResolvedValueOnce(okResponse(remoteItem))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: false, error: 'too large' }, false, 413))
      .mockResolvedValueOnce(okResponse(remoteItem))

    // Not approved: the save already moved `video` to the new hash, so only the caller knows the old one.
    const smartItem = { ...item, video: 'QmNewVideo', contents: { ...item.contents, 'video.mp4': 'QmNewVideo' } }
    await expect(
      saveItem(
        ADDRESS,
        smartItem,
        { 'male/model.glb': new Blob(['model']), 'video.mp4': new Blob(['video']) },
        {
          previousVideo: 'QmOldVideo'
        }
      )
    ).rejects.toMatchObject({ status: 413 })

    const [, , , init] = signedFetchMock.mock.calls[3] as [string, string, string, RequestInit]
    const body = JSON.parse(init.body as string) as { item: { video?: string; contents: Record<string, string> } }
    expect(body.item.video).toBe('QmOldVideo')
    expect(body.item.contents['video.mp4']).toBe('QmOldVideo')
  })

  it('keeps the approved video when the creator re-picks that same file and the upload fails', async () => {
    signedFetchMock
      .mockResolvedValueOnce(okResponse(remoteItem))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: false, error: 'too large' }, false, 413))
      .mockResolvedValueOnce(okResponse(remoteItem))

    const approved = {
      ...item,
      isPublished: true,
      isApproved: true,
      video: 'QmApproved',
      contents: { ...item.contents, 'video.mp4': 'QmApproved' }
    }
    await expect(
      saveItem(ADDRESS, approved, { 'male/model.glb': new Blob(['model']), 'video.mp4': new Blob(['video']) })
    ).rejects.toMatchObject({ status: 413 })

    const [, , , init] = signedFetchMock.mock.calls[3] as [string, string, string, RequestInit]
    const body = JSON.parse(init.body as string) as { item: { video?: string; contents: Record<string, string> } }
    expect(body.item.video).toBe('QmApproved')
    expect(body.item.contents['video.mp4']).toBe('QmApproved')
  })

  it('still reports the video upload error when the rollback PUT fails too', async () => {
    signedFetchMock
      .mockResolvedValueOnce(okResponse(remoteItem))
      .mockResolvedValueOnce(jsonResponse({ ok: true }))
      .mockResolvedValueOnce(jsonResponse({ ok: false, error: 'too large' }, false, 413))
      .mockResolvedValueOnce(jsonResponse({ ok: false, error: 'down' }, false, 500))

    const smartItem = { ...item, video: 'QmVideo', contents: { ...item.contents, 'video.mp4': 'QmVideo' } }
    await expect(
      saveItem(ADDRESS, smartItem, { 'male/model.glb': new Blob(['model']), 'video.mp4': new Blob(['video']) })
    ).rejects.toMatchObject({ status: 413 })
  })

  it('skips the files request when there is nothing to upload', async () => {
    signedFetchMock.mockResolvedValueOnce(okResponse(remoteItem))
    await saveItem(ADDRESS, item, {})
    expect(signedFetchMock).toHaveBeenCalledTimes(1)
  })

  it('surfaces the server error with its status', async () => {
    signedFetchMock.mockResolvedValueOnce(jsonResponse({ ok: false, error: 'locked' }, false, 423))
    await expect(saveItem(ADDRESS, item, {})).rejects.toMatchObject({ status: 423 })
  })
})
