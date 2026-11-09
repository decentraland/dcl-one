import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { primeRemoteFlags, REMOTE_FLAG, remoteFlag, remoteFlagVariant } from '../../shop/discovery/lib/remoteFlags'
import { flagQueries, useShopFlag } from '../features/shop/flagsQuery'

function host(): { client: QueryClient; wrapper: ({ children }: { children: React.ReactNode }) => React.JSX.Element } {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return { client, wrapper: ({ children }) => <QueryClientProvider client={client}>{children}</QueryClientProvider> }
}

afterEach(() => {
  vi.unstubAllGlobals()
  primeRemoteFlags(null)
})

describe('the Shop rollout flags', () => {
  it('reads the web Shop key from the feature-flags file', async () => {
    const f = vi.fn(async (_input: RequestInfo | URL) => Response.json({ flags: { 'dapps-shop-creator-sales': true } }))
    vi.stubGlobal('fetch', f)
    const { wrapper } = host()
    const { result } = renderHook(() => useShopFlag(REMOTE_FLAG.creatorSales), { wrapper })
    // Not known until the file answers.
    expect(result.current).toBeUndefined()
    await waitFor(() => expect(result.current).toBe(true))
    expect(String(f.mock.calls[0][0])).toMatch(/\/dapps\.json$/)
    // The web Shop's fetchers read the same answer from their module.
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(true)
  })

  it("hands the web Shop the variants too, so a page it opens next keeps My Store's address list", async () => {
    const list = `0x${'a'.repeat(40)}`
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        Response.json({
          flags: { 'dapps-shop-my-store': true },
          variants: { 'dapps-shop-my-store': { name: 'testers', enabled: true, payload: { type: 'string', value: list } } }
        })
      )
    )
    const { wrapper } = host()
    const { result } = renderHook(() => useShopFlag(REMOTE_FLAG.myStore), { wrapper })
    await waitFor(() => expect(result.current).toBe(true))
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(list)
  })

  it('is off when the file cannot be read', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 503 })))
    const { client, wrapper } = host()
    const { result } = renderHook(() => useShopFlag(REMOTE_FLAG.creatorSales), { wrapper })
    await waitFor(() => expect(client.getQueryState(flagQueries.dapps().queryKey)?.status).toBe('error'))
    expect(result.current).toBe(false)
  })

  it('keeps its last answer while a refetch fails', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ flags: { 'dapps-shop-creator-sales': true } })))
    const { client, wrapper } = host()
    const { result } = renderHook(() => useShopFlag(REMOTE_FLAG.creatorSales), { wrapper })
    await waitFor(() => expect(result.current).toBe(true))
    vi.stubGlobal('fetch', vi.fn(async () => new Response('', { status: 503 })))
    await act(() => client.refetchQueries({ queryKey: flagQueries.dapps().queryKey }))
    expect(result.current).toBe(true)
  })
})
