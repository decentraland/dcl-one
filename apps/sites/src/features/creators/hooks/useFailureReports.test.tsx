// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { QueryClient, QueryClientProvider, useMutation, useQuery } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { privateKey } from '@/data/lib/creators/queryKeys'
import { useFailureReports } from './useFailureReports'

const captureError = vi.hoisted(() => vi.fn())
vi.mock('@/data/lib/creators/monitoring', () => ({ captureError }))

let client: QueryClient
const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={client}>{children}</QueryClientProvider>
)

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  captureError.mockReset()
})

const failing = (queryKey: readonly unknown[], error: unknown = new Error('down')) =>
  renderHook(
    () => {
      useFailureReports()
      return useQuery({ queryKey, queryFn: () => Promise.reject(error) })
    },
    { wrapper }
  )

describe('useFailureReports', () => {
  it('reports a failed query by its domain, without the account or the inputs of its key', async () => {
    const { result } = failing(privateKey('0xA', 'collection', 'c1'))
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'query', query_key: 'collection' })
  })

  it('leaves the failures of the site chrome and a dismissed wallet prompt alone', async () => {
    const chrome = failing(['private', '0xa', 'chrome', 'notifications'])
    const rejected = failing(['balance'], { code: 4001, message: 'User rejected the request.' })
    await waitFor(() => expect(chrome.result.current.isError && rejected.result.current.isError).toBe(true))
    expect(captureError).not.toHaveBeenCalled()
  })

  it('reports a failed mutation', async () => {
    const { result } = renderHook(
      () => {
        useFailureReports()
        return useMutation({ mutationKey: ['approve'], mutationFn: () => Promise.reject(new Error('reverted')) })
      },
      { wrapper }
    )
    result.current.mutate()
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'mutation', mutation_key: 'approve' })
  })
})
