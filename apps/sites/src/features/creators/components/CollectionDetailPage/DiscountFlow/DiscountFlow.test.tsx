import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { type SaleableCollection } from '~/lib/saleableCollection'
import { DiscountError, type CollectionDiscount } from '~/lib/coupons'
import { Providers, collection, makeSession } from '../SellItemFlow/testUtils'
import { CollectionDiscounts } from './CollectionDiscounts'
import { DiscountFlow } from './DiscountFlow'
import { EndDiscountFlow } from './EndDiscountFlow'

type Callbacks = { onSuccess?: () => void; onError?: (error: unknown) => void }
type Variables = { onSigned?: () => void; discountPct?: number; uses?: number; endsAtMs?: number }
const create = {
  mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(),
  mutateAsync: vi.fn(),
  isPending: false,
  isSuccess: false
}
const end = { mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(), isPending: false }
vi.mock('~/hooks/useCollectionDiscounts', () => ({
  useCreateDiscount: () => create,
  useEndDiscount: () => end,
  useDiscountInFlight: () => ({ creating: create.isPending, ending: end.isPending }),
  endOutcome: () => undefined,
  lastCreatedDiscount: () => undefined
}))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
vi.mock('~/components/CollectionMosaic', () => ({ CollectionMosaic: () => null }))

const DAY = 24 * 60 * 60 * 1000

beforeEach(() => {
  create.mutate.mockReset()
  create.mutateAsync.mockReset()
  end.mutate.mockReset()
})
const lastEnd = () => end.mutate.mock.calls[end.mutate.mock.calls.length - 1]

const sale: SaleableCollection = {
  contractAddress: '0xcc',
  collectionId: collection.id,
  itemCount: 1,
  name: collection.name,
  listedCount: 1,
  items: [{ key: 'k1', name: 'Pirate Hat', thumbnail: '', priceCredits: 100, state: 'discounted', remainingSupply: 9 }]
}

function renderCreate() {
  const onClose = vi.fn()
  render(<DiscountFlow collection={collection} session={makeSession()} sale={sale} onClose={onClose} />, {
    wrapper: Providers
  })
  return { onClose }
}

describe('when a creator creates a discount', () => {
  it("should open the Shop's discount modal for the collection and sign through the create flow", async () => {
    create.mutateAsync.mockResolvedValue({
      id: 'd1',
      status: 'active',
      discount: 200_000,
      collections: ['0xcc'],
      checks: { effective: Date.now(), expiration: Date.now() + 3 * DAY }
    })
    renderCreate()
    expect(screen.getByTestId('creator-sale-modal')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('creator-sale-continue'))
    await userEvent.click(screen.getByTestId('creator-sale-submit'))
    expect(create.mutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({ collection, discountPct: 20, discountedCount: 1 })
    )
    expect(await screen.findByTestId('creator-sale-success-modal')).toBeInTheDocument()
  })

  // Signed, but whether it was stored is unknown: the flow settles that coupon instead of offering a new one.
  it('should switch to settling the signed coupon when storing it got no answer', async () => {
    create.mutateAsync.mockRejectedValue(new DiscountError('unconfirmed'))
    renderCreate()
    await userEvent.click(screen.getByTestId('creator-sale-continue'))
    await userEvent.click(screen.getByTestId('creator-sale-submit'))
    expect(await screen.findByTestId('discount-unconfirmed-modal')).toBeInTheDocument()
  })
})

const running = {
  id: 'coupon-1',
  collections: ['0xcc'],
  discount: 300_000,
  checks: { effective: Date.now() - DAY, expiration: Date.now() + 3 * DAY, uses: 1_000_000 },
  status: 'active',
  state: null
} as unknown as CollectionDiscount

describe("when a creator looks at the collection's discounts", () => {
  it('should show each running or scheduled discount with a way to end it', async () => {
    const onEnd = vi.fn()
    const scheduled = {
      ...running,
      id: 'coupon-2',
      checks: { effective: Date.now() + DAY, expiration: Date.now() + 5 * DAY, uses: 40 },
      state: { uses: 0 }
    } as unknown as CollectionDiscount
    render(<CollectionDiscounts discounts={[running, scheduled]} now={Date.now()} onEnd={onEnd} />, {
      wrapper: Providers
    })

    const chips = screen.getAllByTestId('collection-discount')
    expect(chips[0]).toHaveTextContent('-30%')
    expect(chips[0]).toHaveTextContent(/Ends/)
    expect(chips[1]).toHaveAttribute('data-status', 'scheduled')
    expect(chips[1]).toHaveTextContent('0 of 40 used')
    expect(screen.queryByTestId('discount-other-collections')).not.toBeInTheDocument()
    await userEvent.click(screen.getAllByTestId('end-discount')[1])
    expect(onEnd).toHaveBeenCalledWith(scheduled)
  })

  // A social login cannot move its wallet to Polygon or pay the gas, so its discounts run until their date.
  it('should list the discounts without a way to end them when the creator cannot', () => {
    render(<CollectionDiscounts discounts={[running]} now={Date.now()} />, { wrapper: Providers })
    expect(screen.getByTestId('collection-discount')).toHaveTextContent('-30%')
    expect(screen.queryByTestId('end-discount')).not.toBeInTheDocument()
  })
})

describe('when a creator ends a discount', () => {
  function renderEnd(discount = running) {
    const onClose = vi.fn()
    render(<EndDiscountFlow discount={discount} collection={collection} session={makeSession()} onClose={onClose} />, {
      wrapper: Providers
    })
    return { onClose }
  }

  it('should say ending a discount over several collections ends it on all of them', () => {
    renderEnd({ ...running, collections: ['0xcc', '0xdd', '0xee'] })
    expect(screen.getByTestId('end-discount-modal-description')).toHaveTextContent('2 other collections')
  })

  it('should warn it is a paid transaction, send it and celebrate', async () => {
    renderEnd()
    expect(screen.getByTestId('end-discount-modal-description')).toHaveTextContent(/30% discount.*POL/)
    await userEvent.click(screen.getByTestId('end-discount-confirm'))
    expect(lastEnd()[0]).toMatchObject({ discount: running, collection })
    await act(async () => lastEnd()[0].onSigned?.())
    expect(screen.getByTestId('end-discount-pending-label')).toHaveTextContent(/ending your discount/i)
    await act(async () => lastEnd()[1].onSuccess?.())
    expect(screen.getByTestId('success-modal-title')).toHaveTextContent(/discount ended/i)
  })

  it('should not offer a second paid cancellation while the first one is still mining', async () => {
    renderEnd()
    await userEvent.click(screen.getByTestId('end-discount-confirm'))
    await act(async () => lastEnd()[1].onError?.(new DiscountError('pending')))
    expect(screen.getByTestId('end-discount-slow-modal-description')).toHaveTextContent(/don't send it again/i)
    expect(screen.queryByTestId('end-discount-error-retry')).not.toBeInTheDocument()
  })

  it('should return to the confirmation on a dismissed wallet and offer a retry on a failure', async () => {
    renderEnd()
    await userEvent.click(screen.getByTestId('end-discount-confirm'))
    await act(async () => lastEnd()[1].onError?.({ code: 4001 }))
    expect(screen.getByTestId('end-discount-modal')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('end-discount-confirm'))
    await act(async () => lastEnd()[1].onError?.(new Error('insufficient funds')))
    expect(screen.getByTestId('end-discount-error-modal-description')).toHaveTextContent(/POL/)
    await userEvent.click(screen.getByTestId('end-discount-error-retry'))
    expect(screen.getByTestId('end-discount-modal')).toBeInTheDocument()
  })
})
