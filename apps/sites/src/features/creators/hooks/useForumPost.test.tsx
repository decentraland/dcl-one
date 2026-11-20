// @vitest-environment jsdom
import { privateKey } from '@/data/lib/creators/queryKeys'
import '@/data/lib/creators/test/setup'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import type { Collection } from '@/data/lib/creators/collections'
import type { Item } from '@/data/lib/creators/items'

const api = vi.hoisted(() => ({
  createCollectionForumPost: vi.fn(),
  createCurationForumReply: vi.fn(),
  fetchCollection: vi.fn()
}))
vi.mock('@/data/lib/creators/builder', async importOriginal => ({ ...(await importOriginal<object>()), ...api }))
vi.mock('@/data/lib/creators/analytics', () => ({ track: vi.fn(), errorCode: () => 'unknown' }))
vi.mock('@/data/lib/creators/monitoring', () => ({ captureError: vi.fn() }))

const { postToForum, useForumPostRecovery } = await import('./useForumPost')

const OWNER = '0x00000000000000000000000000000000000000aa'
const LINK = 'https://forum.decentraland.org/t/hats/77'
const collection = (id: string) =>
  ({
    id,
    owner: OWNER,
    isPublished: true,
    isApproved: false,
    managers: [],
    minters: [],
    updatedAt: Date.now() - 8 * 60 * 60_000
  }) as unknown as Collection
const synced = [{ id: 'i1', tokenId: '1' }] as Item[]

let client: QueryClient
function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  api.createCollectionForumPost.mockReset().mockResolvedValue(LINK)
  api.fetchCollection.mockReset().mockResolvedValue({ forumLink: undefined })
})

describe('postToForum', () => {
  it('writes the new topic link into the cached collection and refreshes the lists', async () => {
    const queue = privateKey(OWNER, 'curation-collections', {})
    client.setQueryData(queue, [])
    client.setQueryData(privateKey(OWNER, 'collection', 'h1'), collection('h1'))

    await postToForum(client, OWNER, collection('h1'), 'publish')

    expect(api.createCollectionForumPost).toHaveBeenCalledWith(OWNER, 'h1')
    expect(client.getQueryData<Collection>(privateKey(OWNER, 'collection', 'h1'))?.forumLink).toBe(LINK)
    expect(client.getQueryState(queue)?.isInvalidated).toBe(true)
  })
})

describe('useForumPostRecovery', () => {
  it('posts once, across rerenders and remounts', async () => {
    const subject = collection('h2')
    const { rerender, unmount } = renderHook(() => useForumPostRecovery(OWNER, { ...subject }, synced), { wrapper })
    rerender()
    unmount()
    renderHook(() => useForumPostRecovery(OWNER, { ...subject }, synced), { wrapper })

    await waitFor(() => expect(api.createCollectionForumPost).toHaveBeenCalledTimes(1))
  })
})
