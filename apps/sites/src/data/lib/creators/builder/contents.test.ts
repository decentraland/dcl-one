import { describe, it, expect, vi } from 'vitest'
import { type Item } from '~/lib/items'
import { fetchContentSize, fetchItemContents, getContentsStorageUrl } from './contents'

describe('getContentsStorageUrl', () => {
  it('builds the storage URL for a content hash', () => {
    expect(getContentsStorageUrl('Qmx')).toBe('https://builder-api.decentraland.zone/v1/storage/contents/Qmx')
  })
})

describe('fetchItemContents', () => {
  const item = {
    id: 'item-1',
    contents: { 'male/model.glb': 'QmModel', 'thumbnail.png': 'QmThumb' }
  } as unknown as Item

  it('downloads every file from public storage keyed by its path', async () => {
    const fetchMock = vi.fn(async (url: string) => new Response(url.endsWith('QmModel') ? 'model' : 'thumb'))
    vi.stubGlobal('fetch', fetchMock)
    const contents = await fetchItemContents(item)
    expect(Object.keys(contents).sort()).toEqual(['male/model.glb', 'thumbnail.png'])
    expect(await contents['male/model.glb'].text()).toBe('model')
    expect(fetchMock.mock.calls.map(([url]) => url)).toContain(getContentsStorageUrl('QmThumb'))
    vi.unstubAllGlobals()
  })

  it('fails when any file cannot be downloaded', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(null, { status: 404 }))
    )
    await expect(fetchItemContents(item)).rejects.toThrow(/404/)
    vi.unstubAllGlobals()
  })
})

describe('fetchContentSize', () => {
  it('reads the size storage declares', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('abc', { headers: { 'content-length': '2048' } }))
    )
    expect(await fetchContentSize('bafmodel')).toBe(2048)
    vi.unstubAllGlobals()
  })

  it('measures the body when storage does not declare a size, so a file never counts as empty', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(new Uint8Array(1500)))
    )
    expect(await fetchContentSize('bafmodel')).toBe(1500)
    vi.unstubAllGlobals()
  })

  it('fails when the file cannot be read', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(null, { status: 404 }))
    )
    await expect(fetchContentSize('bafmodel')).rejects.toThrow(/404/)
    vi.unstubAllGlobals()
  })

  it('gives up when its signal fires, so a stalled request never stays pending', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(
        (_url: string, init: RequestInit) =>
          new Promise((_resolve, reject) => init.signal?.addEventListener('abort', () => reject(new Error('aborted'))))
      )
    )
    const timeout = new AbortController()
    const size = fetchContentSize('bafmodel', timeout.signal)
    timeout.abort()
    await expect(size).rejects.toThrow('aborted')
    vi.unstubAllGlobals()
  })
})
