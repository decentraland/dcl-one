import { type ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, render, renderHook, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type CollectionDiscount, type CouponCreation } from '~/lib/coupons'
import { collection, makeSession } from '~/components/CollectionDetailPage/SellItemFlow/testUtils'

vi.mock('~/lib/auth', async () => {
  const actual = await vi.importActual<typeof import('~/lib/auth')>('~/lib/auth')
  return { ...actual, readTransactionOutcome: vi.fn() }
})
vi.mock('~/lib/coupons', async () => {
  const actual = await vi.importActual<typeof import('~/lib/coupons')>('~/lib/coupons')
  return {
    ...actual,
    fetchCreatorDiscounts: vi.fn(),
    createDiscount: vi.fn(),
    endDiscount: vi.fn(),
    storeSignedDiscount: vi.fn()
  }
})
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
vi.mock('~/components/CollectionMosaic', () => ({ CollectionMosaic: () => null }))

import { readTransactionOutcome } from '~/lib/auth'
import { createDiscount, endDiscount, fetchCreatorDiscounts, storeSignedDiscount } from '~/lib/coupons'
import { savePendingEnd, saveUnconfirmedCreate, type PendingEnd } from '~/lib/discountAttempts'
import { DiscountFlow } from '~/components/CollectionDetailPage/DiscountFlow/DiscountFlow'
import { EndDiscountFlow } from '~/components/CollectionDetailPage/DiscountFlow/EndDiscountFlow'
import {
  pollEvery,
  reconcilePendingEnds,
  resetDiscountSession,
  useCollectionDiscounts,
  useCreateDiscount,
  useEndDiscount
} from './useCollectionDiscounts'

const DAY = 24 * 60 * 60 * 1000
const session = makeSession()

const running = {
  id: 'running',
  signer: session.address,
  chainId: 80002,
  collections: [collection.contractAddress],
  discountType: 1,
  discount: 300_000,
  status: 'active',
  checks: { effective: Date.now() - DAY, expiration: Date.now() + DAY, uses: 1_000_000 }
} as unknown as CollectionDiscount

// A fresh client stands in for a reload: nothing in memory, only what the browser kept.
function freshPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
}

afterEach(() => {
  resetDiscountSession()
  vi.mocked(readTransactionOutcome).mockReset()
  vi.mocked(fetchCreatorDiscounts).mockReset()
  vi.mocked(endDiscount).mockReset()
  vi.mocked(createDiscount).mockReset()
  vi.mocked(storeSignedDiscount).mockReset()
})

const sendEnd = (patch: Partial<PendingEnd> = {}) =>
  savePendingEnd(session.address, {
    discountId: 'running',
    txHash: '0xhash',
    nonce: 4,
    chainId: 80002,
    sentAt: Date.now(),
    expiresAt: Date.now() + DAY,
    ...patch
  })

const listFor = () =>
  renderHook(() => useCollectionDiscounts(session.address, collection.contractAddress), { wrapper: freshPage() })

describe('when a cancellation was sent but not seen mined before a reload', () => {
  it('should not offer End again after the reload, sending nothing', () => {
    sendEnd()
    render(<EndDiscountFlow discount={running} collection={collection} session={session} onClose={vi.fn()} />, {
      wrapper: freshPage()
    })

    expect(screen.getByTestId('end-discount-slow-modal')).toBeInTheDocument()
    expect(screen.queryByTestId('end-discount-confirm')).not.toBeInTheDocument()
    expect(endDiscount).not.toHaveBeenCalled()
  })

  // Hidden while it still applies would be wrong; offering End again would pay for a second cancellation.
  it('should keep the discount listed as ending while the chain has no receipt for it', async () => {
    sendEnd()
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])
    vi.mocked(readTransactionOutcome).mockResolvedValue('unknown')

    const { result } = listFor()

    await waitFor(() =>
      expect(result.current.discounts).toEqual([expect.objectContaining({ id: 'running', ending: true })])
    )
  })

  it('should bring End back once the chain says the cancellation reverted', async () => {
    sendEnd()
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])
    vi.mocked(readTransactionOutcome).mockResolvedValue('reverted')

    const { result } = listFor()

    await waitFor(() => expect(readTransactionOutcome).toHaveBeenCalled())
    await waitFor(() => expect(result.current.discounts?.[0]?.ending).toBeUndefined())
  })

  it('should take the discount off the list once the chain says the cancellation mined', async () => {
    sendEnd()
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])
    vi.mocked(readTransactionOutcome).mockResolvedValue('mined')

    const { result } = listFor()

    await waitFor(() => expect(result.current.discounts).toEqual([]))
  })
})

describe('when a recorded cancellation has no receipt for a long time', () => {
  // A low-fee or nonce-blocked transaction can still mine; age alone proves nothing.
  it('should keep End disabled however old it is, while its nonce is unused', async () => {
    sendEnd({ sentAt: Date.now() - 7 * DAY })
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])
    vi.mocked(readTransactionOutcome).mockResolvedValue('unknown')
    const { result } = listFor()
    await waitFor(() => expect(readTransactionOutcome).toHaveBeenCalled())
    expect(result.current.discounts).toEqual([expect.objectContaining({ ending: true })])
  })

  it('should bring End back only once the server read the coupon after its nonce went elsewhere', async () => {
    const supersededAt = Date.now() - 60_000
    sendEnd({ supersededAt })
    const readAt = (checkedAt: number) => ({
      ...running,
      state: { uses: 0, cancelled: false, revoked: false, checkedAt }
    })

    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([readAt(supersededAt - 1)])
    const before = listFor()
    await waitFor(() => expect(before.result.current.discounts).toEqual([expect.objectContaining({ ending: true })]))
    before.unmount()

    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([readAt(supersededAt + 1)])
    const after = listFor()
    await waitFor(() => expect(after.result.current.discounts?.[0]?.ending).toBeUndefined())
  })
})

describe('when a discount is ended before the server lists it', () => {
  // Create, then End straight away: the last server list predates the coupon, which proves nothing about it.
  it('should keep End away after a reload once the server does list it, still running', async () => {
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([])
    vi.mocked(createDiscount).mockResolvedValue(running)
    vi.mocked(endDiscount).mockImplementation(async (_, deps) => {
      deps.onBroadcast?.('0xhash', 4)
    })
    const page = renderHook(
      () => ({
        list: useCollectionDiscounts(session.address, collection.contractAddress),
        create: useCreateDiscount(session),
        end: useEndDiscount(session)
      }),
      { wrapper: freshPage() }
    )
    await waitFor(() => expect(page.result.current.list.discounts).toEqual([]))

    await act(async () => {
      await page.result.current.create.mutateAsync({
        collection,
        discountPct: 30,
        endsAtMs: Date.now() + DAY,
        discountedCount: 1
      })
    })
    await act(async () => {
      await page.result.current.end.mutateAsync({ discount: running, collection })
    })
    await new Promise(resolve => setTimeout(resolve, 50))
    page.unmount()

    vi.resetModules()
    const reloaded = await import('./useCollectionDiscounts')
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])
    const after = renderHook(() => reloaded.useCollectionDiscounts(session.address, collection.contractAddress), {
      wrapper: freshPage()
    })
    await waitFor(() => expect(after.result.current.isSuccess).toBe(true))
    expect(after.result.current.discounts).toEqual([])
    expect(reloaded.endOutcome(session.address, 'running')).toBe('ended')
  })

  it('should keep a cancellation recorded when the discount is missing from the list it is checked against', async () => {
    sendEnd()
    vi.mocked(readTransactionOutcome).mockResolvedValue('unknown')
    await expect(reconcilePendingEnds(session.address, [])).resolves.toEqual({ ending: ['running'], ended: [] })
    expect(readTransactionOutcome).toHaveBeenCalled()
  })
})

describe('when a cancellation mined but the server still reports the discount running', () => {
  // The app's own display patch must not read as the server catching up.
  it('should keep End away after a real End, a reconcile and a reload, while the server still says running', async () => {
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])
    vi.mocked(endDiscount).mockImplementation(async (_, deps) => {
      deps.onBroadcast?.('0xhash', 4)
    })
    const wrapper = freshPage()
    const page = renderHook(
      () => ({
        list: useCollectionDiscounts(session.address, collection.contractAddress),
        end: useEndDiscount(session)
      }),
      { wrapper }
    )
    await waitFor(() => expect(page.result.current.list.discounts).toHaveLength(1))

    await act(async () => {
      await page.result.current.end.mutateAsync({ discount: running, collection })
    })
    await waitFor(() => expect(page.result.current.list.discounts).toEqual([]))
    await new Promise(resolve => setTimeout(resolve, 50))
    page.unmount()

    vi.resetModules()
    const reloaded = await import('./useCollectionDiscounts')
    const after = renderHook(() => reloaded.useCollectionDiscounts(session.address, collection.contractAddress), {
      wrapper: freshPage()
    })
    await waitFor(() => expect(after.result.current.isSuccess).toBe(true))
    expect(after.result.current.discounts).toEqual([])
    expect(reloaded.endOutcome(session.address, 'running')).toBe('ended')
  })

  // The server reads coupon state on its own schedule; a reload in between must not offer End again.
  it('should keep it ended across a reload, with nothing but the browser storage left', async () => {
    sendEnd({ minedAt: Date.now() })
    vi.resetModules()
    const reloaded = await import('./useCollectionDiscounts')
    vi.mocked(fetchCreatorDiscounts).mockResolvedValue([running])

    const { result } = renderHook(() => reloaded.useCollectionDiscounts(session.address, collection.contractAddress), {
      wrapper: freshPage()
    })

    await waitFor(() => expect(result.current.discounts).toEqual([]))
    expect(reloaded.endOutcome(session.address, 'running')).toBe('ended')
    expect(readTransactionOutcome).not.toHaveBeenCalled()
  })
})

describe('when a signed discount was posted with no readable answer before a reload', () => {
  const payload = {
    signer: session.address,
    chainId: 80002,
    signature: '0xsigned',
    discount: 300_000,
    collections: [collection.contractAddress],
    checks: { effective: Date.now(), expiration: Date.now() + DAY, uses: 1_000_000 }
  } as unknown as CouponCreation

  // A second signature would put a second, independently redeemable coupon on sale.
  it('should settle that same coupon instead of asking for a new signature', async () => {
    saveUnconfirmedCreate(session.address, { collectionId: collection.id, payload })
    vi.mocked(storeSignedDiscount).mockResolvedValue({ ...running, id: 'stored' })
    const sale = {
      contractAddress: '0xcc',
      collectionId: collection.id,
      itemCount: 1,
      name: 'x',
      listedCount: 1,
      items: []
    }

    render(<DiscountFlow collection={collection} session={session} sale={sale} onClose={vi.fn()} />, {
      wrapper: freshPage()
    })
    expect(screen.queryByTestId('creator-sale-modal')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('discount-unconfirmed-retry'))

    expect(await screen.findByTestId('success-modal-title')).toHaveTextContent(/your discount is live/i)
    expect(storeSignedDiscount).toHaveBeenCalledWith(payload, expect.anything())
    expect(createDiscount).not.toHaveBeenCalled()
  })
})

describe('when deciding how often to re-check recorded cancellations', () => {
  const end = (sentAt: number) => ({ discountId: 'd', txHash: '0x', chainId: 80002, sentAt, expiresAt: sentAt + DAY })
  const now = Date.now()

  it('should check often while one is fresh, back off once all are old, and stop with none', () => {
    expect(pollEvery([end(now - 60_000)], now)).toBe(30_000)
    expect(pollEvery([end(now - 60 * 60_000)], now)).toBe(5 * 60_000)
    expect(pollEvery([end(now - 60 * 60_000), end(now - 60_000)], now)).toBe(30_000)
    expect(pollEvery([], now)).toBe(false)
  })
})
