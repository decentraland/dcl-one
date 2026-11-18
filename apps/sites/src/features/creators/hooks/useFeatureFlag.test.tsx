import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { FeatureFlag } from '~/lib/featureFlags'

const getIsFeatureEnabled = vi.fn()
vi.mock('~/lib/featureFlags', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/featureFlags')>()),
  getIsFeatureEnabled: (flag: FeatureFlag) => getIsFeatureEnabled(flag)
}))

const { useFeatureFlag } = await import('./useFeatureFlag')

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => getIsFeatureEnabled.mockReset())

describe('useFeatureFlag', () => {
  it('reports the flag as off while it is still being read, then as the service answers', async () => {
    getIsFeatureEnabled.mockResolvedValue(true)
    const { result } = renderHook(() => useFeatureFlag(FeatureFlag.WEARABLE_UTILITY), { wrapper })
    expect(result.current).toEqual({ enabled: false, isLoading: true })
    await waitFor(() => expect(result.current).toEqual({ enabled: true, isLoading: false }))
    expect(getIsFeatureEnabled).toHaveBeenCalledWith(FeatureFlag.WEARABLE_UTILITY)
  })

  // An unreachable service reads as `false` one layer down, in `lib/featureFlags`.
  it('stays off when the flag is off', async () => {
    getIsFeatureEnabled.mockResolvedValue(false)
    const { result } = renderHook(() => useFeatureFlag(FeatureFlag.MAINTENANCE), { wrapper })
    await waitFor(() => expect(result.current.isLoading).toBe(false))
    expect(result.current.enabled).toBe(false)
  })

  it('reads nothing and settles at once when the caller has no use for the answer', () => {
    const { result } = renderHook(() => useFeatureFlag(FeatureFlag.VRM_OPTOUT, { enabled: false }), { wrapper })
    expect(result.current).toEqual({ enabled: false, isLoading: false })
    expect(getIsFeatureEnabled).not.toHaveBeenCalled()
  })
})
