import { describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'

const fetchCommittee = vi.hoisted(() => vi.fn())
vi.mock('~/lib/builder', () => ({ fetchCommittee }))

const { useCommittee } = await import('./useCommittee')

const ADDRESS = '0xCurator'

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

describe('useCommittee', () => {
  it('is not a curator until the committee loads, then matches the wallet case-insensitively', async () => {
    fetchCommittee.mockResolvedValue(['0xcurator'])
    const { result } = renderHook(() => useCommittee(ADDRESS), { wrapper })
    expect(result.current.isCurator).toBe(false)
    await waitFor(() => expect(result.current.isCurator).toBe(true))
  })

  it('fails closed when the committee cannot be read', async () => {
    fetchCommittee.mockRejectedValue(new Error('down'))
    const { result } = renderHook(() => useCommittee(ADDRESS), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.isCurator).toBe(false)
  })
})
