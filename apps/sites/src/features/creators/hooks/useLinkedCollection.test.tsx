import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { CurationStatus, type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { LinkedItemFilter } from '~/lib/linkedItemStatus'
import { useLinkedCollectionItems } from './useLinkedCollection'

vi.mock('~/lib/builder', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/builder')>()),
  fetchAllLinkedCollectionItems: vi.fn(),
  fetchItemCurations: vi.fn()
}))

import { fetchAllLinkedCollectionItems, fetchItemCurations } from '~/lib/builder'

const collection = { id: 'c1', urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats' } as Collection
const items = ['1', '2', '3', '4', '5'].map(id => ({ id, name: `Hat ${id}` }) as Item)

describe('useLinkedCollectionItems under Draft', () => {
  it('reads the collection once and pages through the drafts from that one read', async () => {
    vi.mocked(fetchAllLinkedCollectionItems).mockResolvedValue(items)
    const approved = { id: 'ic5', itemId: '5', status: CurationStatus.APPROVED, contentHash: 'h' }
    vi.mocked(fetchItemCurations).mockResolvedValue(new Map([['5', approved]]) as never)
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    )

    const { result, rerender } = renderHook(
      ({ page }) => useLinkedCollectionItems('0xme', collection, page, LinkedItemFilter.DRAFT, 2),
      { wrapper, initialProps: { page: 1 } }
    )
    await waitFor(() => expect(result.current.data?.results.map(item => item.id)).toEqual(['1', '2']))
    expect(result.current.data).toMatchObject({ total: 4, pages: 2 })

    rerender({ page: 2 })
    await waitFor(() => expect(result.current.data?.results.map(item => item.id)).toEqual(['3', '4']))
    expect(fetchAllLinkedCollectionItems).toHaveBeenCalledTimes(1)
  })
})
