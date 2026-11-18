import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { type Collection, type CollectionsList } from '~/lib/collections'

const api = vi.hoisted(() => ({ deleteCollection: vi.fn() }))
vi.mock('~/lib/builder', async importOriginal => ({ ...(await importOriginal<object>()), ...api }))
vi.mock('~/lib/analytics', async importOriginal => ({ ...(await importOriginal<object>()), track: vi.fn() }))

const { useDeleteCollection } = await import('./useCollection')

const OWNER = '0x00000000000000000000000000000000000000aa'
const kept = { id: 'kept' } as Collection
const deleted = { id: 'deleted' } as Collection

function page(results: Collection[]): CollectionsList {
  return { results, total: results.length, page: 1, pages: 1, limit: 8 }
}

function renderDelete(client: QueryClient) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return renderHook(() => useDeleteCollection(OWNER), { wrapper })
}

let client: QueryClient

beforeEach(() => {
  api.deleteCollection.mockReset().mockResolvedValue(undefined)
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
})

describe('when a collection is deleted', () => {
  const listKey = ['collections', OWNER, 1, '', 'all', undefined, 8, false]
  const previewKey = ['collection-preview', OWNER, 'deleted']

  beforeEach(() => {
    client.setQueryData(listKey, page([kept, deleted]))
    client.setQueryData(previewKey, [])
  })

  it('drops it from the cached lists, so the list never renders it while it refetches', async () => {
    const { result } = renderDelete(client)

    await act(() => result.current.mutateAsync('deleted'))

    expect(client.getQueryData<CollectionsList>(listKey)?.results).toStrictEqual([kept])
  })

  it('forgets its cover previews', async () => {
    const { result } = renderDelete(client)

    await act(() => result.current.mutateAsync('deleted'))

    expect(client.getQueryData(previewKey)).toBeUndefined()
  })

  it('still marks the lists stale so they refetch', async () => {
    const { result } = renderDelete(client)

    await act(() => result.current.mutateAsync('deleted'))

    await waitFor(() => expect(client.getQueryState(listKey)?.isInvalidated).toBe(true))
  })
})

describe('when the delete fails', () => {
  it('keeps the collection in the cached lists', async () => {
    api.deleteCollection.mockRejectedValue(new Error('boom'))
    const listKey = ['collections', OWNER, 1, '', 'all', undefined, 8, false]
    client.setQueryData(listKey, page([kept, deleted]))
    const { result } = renderDelete(client)

    await act(() => result.current.mutateAsync('deleted').catch(() => undefined))

    expect(client.getQueryData<CollectionsList>(listKey)?.results).toStrictEqual([kept, deleted])
  })
})
