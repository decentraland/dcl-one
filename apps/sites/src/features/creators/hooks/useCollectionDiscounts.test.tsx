import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type CollectionDiscount } from '~/lib/coupons'
import { Providers, collection, makeSession } from '~/components/CollectionDetailPage/SellItemFlow/testUtils'

vi.mock('~/lib/auth', async () => {
  const actual = await vi.importActual<typeof import('~/lib/auth')>('~/lib/auth')
  return { ...actual, sendWalletTransaction: vi.fn(), waitForTransaction: vi.fn() }
})

vi.mock('~/lib/coupons', async () => {
  const actual = await vi.importActual<typeof import('~/lib/coupons')>('~/lib/coupons')
  return { ...actual, endDiscount: vi.fn() }
})

import { sendWalletTransaction } from '~/lib/auth'
import { endDiscount } from '~/lib/coupons'
import { useDiscountInFlight, useEndDiscount } from './useCollectionDiscounts'

describe('when ending a discount', () => {
  // Cancellations are keyed by the caller: anyone else's cancellation leaves the coupon redeemable.
  it('should refuse a discount another account signed, sending nothing', async () => {
    const { result } = renderHook(() => useEndDiscount(makeSession()), { wrapper: Providers })
    const discount = { id: 'c1', signer: '0x00000000000000000000000000000000000000bb' } as CollectionDiscount

    await expect(result.current.mutateAsync({ discount, collection })).rejects.toThrow(/signed by/)
    expect(sendWalletTransaction).not.toHaveBeenCalled()
  })
})

describe('when a cancellation is still in flight', () => {
  // Closing and reopening the dialog mounts a new mutation; the guard must still see the first one.
  it('should say so to any other dialog for that discount, until it settles', async () => {
    const queryClient = new QueryClient({ defaultOptions: { mutations: { retry: false } } })
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
    let finish: () => void = () => {}
    vi.mocked(endDiscount).mockReturnValue(new Promise<void>(resolve => (finish = resolve)))
    const session = makeSession()
    const discount = { id: 'c1', signer: session.address, chainId: 80002 } as CollectionDiscount

    const first = renderHook(() => useEndDiscount(session), { wrapper })
    const reopened = renderHook(() => useDiscountInFlight({ discountId: 'c1' }), { wrapper })
    const other = renderHook(() => useDiscountInFlight({ discountId: 'c2' }), { wrapper })
    act(() => first.result.current.mutate({ discount, collection }))

    await waitFor(() => expect(reopened.result.current.ending).toBe(true))
    expect(other.result.current.ending).toBe(false)
    finish()
    await waitFor(() => expect(reopened.result.current.ending).toBe(false))
  })
})
