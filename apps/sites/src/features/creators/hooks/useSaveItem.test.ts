import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createElement, type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { fetchContent, fetchContentSize, saveItem } from '~/lib/builder'
import { track } from '~/lib/analytics'
import { captureError } from '~/lib/monitoring'
import { ItemType, type Item } from '~/lib/items'
import { buildSaveItem, fetchStoredThumbnail, useSaveItem, wasImageRefreshSkipped } from './useSaveItem'

vi.mock('~/lib/builder', () => ({ fetchContent: vi.fn(), fetchContentSize: vi.fn(), saveItem: vi.fn() }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn(), errorCode: () => 'unknown' }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))

const item: Item = {
  id: 'w1',
  name: 'Hat',
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  rarity: 'epic',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type: ItemType.WEARABLE,
  data: { category: 'hat', hides: [], replaces: [], tags: [], representations: [] },
  contents: { 'male/hat.glb': 'bafyglb', 'thumbnail.png': 'bafythumb', 'image.png': 'bafyimage' },
  createdAt: 1,
  updatedAt: 1
}
const png = () => new Blob(['png'], { type: 'image/png' })

beforeEach(() => {
  vi.mocked(fetchContent).mockReset()
  vi.mocked(fetchContentSize).mockReset()
  vi.mocked(saveItem).mockReset()
  vi.mocked(captureError).mockReset()
  vi.mocked(track).mockReset()
})

describe('buildSaveItem', () => {
  it('rebuilds only the catalyst image from the stored thumbnail when the rarity went stale', async () => {
    vi.mocked(fetchContent).mockResolvedValue(png())
    const { item: built, blobs } = await buildSaveItem({ item, imageStale: true })
    expect(fetchContent).toHaveBeenCalledWith('bafythumb')
    expect(Object.keys(blobs)).toEqual(['image.png'])
    expect(built.contents['image.png']).not.toBe('bafyimage')
    expect(built.contents['thumbnail.png']).toBe('bafythumb')
  })

  it('lets a new thumbnail win over a stale image: both files come from the new picture', async () => {
    const { blobs } = await buildSaveItem({ item, thumbnail: png(), imageStale: true })
    expect(fetchContent).not.toHaveBeenCalled()
    expect(Object.keys(blobs).sort()).toEqual(['image.png', 'thumbnail.png'])
  })

  it('saves as is when nothing about the picture changed', async () => {
    const { blobs } = await buildSaveItem({ item, blobs: { 'male/hat.glb': new Blob(['glb']) } })
    expect(fetchContent).not.toHaveBeenCalled()
    expect(Object.keys(blobs)).toEqual(['male/hat.glb'])
  })
})

describe('fetchStoredThumbnail', () => {
  it('is null for an item without a stored thumbnail, without touching storage', async () => {
    expect(await fetchStoredThumbnail({ ...item, contents: { 'male/hat.glb': 'bafyglb' } })).toBeNull()
    expect(fetchContent).not.toHaveBeenCalled()
  })

  it('reports a storage failure and lets the save go on without the image', async () => {
    vi.mocked(fetchContent).mockRejectedValue(new Error('storage down'))
    expect(await fetchStoredThumbnail(item)).toBeNull()
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'save-item', itemId: 'w1' })
    const variables = { item, imageStale: true }
    const built = await buildSaveItem(variables)
    expect(built.blobs).toEqual({})
    expect(wasImageRefreshSkipped(variables, built)).toBe(true)
  })

  it('does not count a save with a new thumbnail, or one with the image rebuilt, as skipped', async () => {
    vi.mocked(fetchContent).mockResolvedValue(png())
    const stale = { item, imageStale: true }
    expect(wasImageRefreshSkipped(stale, await buildSaveItem(stale))).toBe(false)
    const fresh = { item, thumbnail: png(), imageStale: true }
    expect(wasImageRefreshSkipped(fresh, await buildSaveItem(fresh))).toBe(false)
  })
})

describe('useSaveItem size guard', () => {
  function renderSave() {
    const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } })
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client }, createElement(TranslationProvider, null, children))
    return renderHook(() => useSaveItem('0xabc'), { wrapper }).result
  }

  it('refuses a save that leaves the item over its limit, before anything is sent', async () => {
    // Bull&Bear Merch: a 1.4 MB model next to the 1.5 MB one, the thumbnail and the rarity image.
    const sizes: Record<string, number> = { bafyglb: 1_537_136, bafythumb: 201_089, bafyimage: 87_467 }
    vi.mocked(fetchContentSize).mockImplementation(async hash => sizes[hash])
    const save = renderSave()
    const update = {
      item: { ...item, contents: { ...item.contents, 'female/hat.glb': 'bafynew' } },
      blobs: { 'female/hat.glb': new Blob([new Uint8Array(1_417_980)]) }
    }
    await expect(save.current.mutateAsync(update)).rejects.toMatchObject({
      messageKey: 'size_exceeded',
      messageParams: { size: '3.09', limit: '3.00' }
    })
    expect(saveItem).not.toHaveBeenCalled()
  })

  it('saves an edit that changes neither the files nor the size limit of an item already over it', async () => {
    vi.mocked(fetchContentSize).mockResolvedValue(2 * 1024 * 1024)
    vi.mocked(saveItem).mockResolvedValue(item)
    const save = renderSave()
    await expect(save.current.mutateAsync({ item: { ...item, name: 'Cap' }, previous: item })).resolves.toBe(item)
    expect(saveItem).toHaveBeenCalledTimes(1)
  })

  it("counts a save as the user's unless the app made it on its own", async () => {
    vi.mocked(saveItem).mockResolvedValue(item)
    const save = renderSave()
    await save.current.mutateAsync({ item, background: true })
    expect(track).not.toHaveBeenCalled()
    await save.current.mutateAsync({ item })
    expect(track).toHaveBeenCalledWith('Save item', { itemId: item.id })
  })

  it('refuses a change of category that moves an item to a lower limit it is over', async () => {
    const skin: Item = { ...item, data: { ...item.data, category: 'skin' } }
    vi.mocked(fetchContentSize).mockResolvedValue(2 * 1024 * 1024)
    const save = renderSave()
    await expect(save.current.mutateAsync({ item: item, previous: skin })).rejects.toMatchObject({
      messageKey: 'size_exceeded',
      messageParams: { limit: '3.00' }
    })
    expect(saveItem).not.toHaveBeenCalled()
  })

  it('saves through, reported, when storage cannot be measured', async () => {
    vi.mocked(fetchContentSize).mockRejectedValue(new Error('storage down'))
    vi.mocked(saveItem).mockResolvedValue(item)
    const save = renderSave()
    await expect(
      save.current.mutateAsync({ item: { ...item, name: 'Cap' }, blobs: { 'thumbnail.png': png() } })
    ).resolves.toBe(item)
    expect(saveItem).toHaveBeenCalledTimes(1)
    expect(captureError).toHaveBeenCalledWith(
      expect.any(Error),
      expect.objectContaining({ flow: 'save-item', step: 'measure_size' })
    )
  })
})
