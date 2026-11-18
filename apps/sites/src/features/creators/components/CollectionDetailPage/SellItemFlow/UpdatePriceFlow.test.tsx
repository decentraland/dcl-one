import { describe, it, expect, vi, beforeEach } from 'vitest'
import { FeatureFlag } from '~/lib/featureFlags'
import { setFeatureFlags } from '~/test/featureFlags'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProviderType } from '@dcl/schemas'
import { type ItemListing } from '~/lib/listings'
import { NO_EXPIRATION } from '~/lib/sales'
import { UpdatePriceFlow } from './UpdatePriceFlow'
import { ADDRESS, Providers, collection, item, makeSession } from './testUtils'

vi.mock('~/lib/featureFlags', async () => {
  const actual = await vi.importActual<typeof import('~/lib/featureFlags')>('~/lib/featureFlags')
  const mock = await import('~/test/featureFlags')
  return { ...actual, getIsFeatureEnabled: mock.getIsFeatureEnabled }
})

beforeEach(() => setFeatureFlags(FeatureFlag.OFFCHAIN_PUBLIC_ITEM_ORDERS, FeatureFlag.CREDITS_PRIMARY_LISTINGS))

type Callbacks = { onSuccess?: (result: unknown) => void; onError?: (error: unknown) => void }
type UpdateVariables = {
  price: unknown
  onSigned?: (step: 'cancel' | 'sign') => void
  onCancelled?: (terms: unknown) => void
}
type SellVariables = { onSigned?: () => void }
const update = { mutate: vi.fn<(variables: UpdateVariables, callbacks: Callbacks) => void>(), isPending: false }
const sell = { mutate: vi.fn<(variables: SellVariables, callbacks: Callbacks) => void>(), isPending: false }
type EnableVariables = { onSigned?: () => void }
const enable = { mutate: vi.fn<(variables: EnableVariables, callbacks: Callbacks) => void>(), isPending: false }
let salesEnabled = true
vi.mock('~/hooks/useSales', () => ({
  useSalesEnabled: () => salesEnabled,
  useEnableSales: () => enable,
  useUpdatePrice: () => update,
  useSellItem: () => sell,
  useManaUsdRate: () => ({ data: undefined })
}))
vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))

const listing: ItemListing & { tradeId: string } = { itemId: '3', tradeId: 'trade-1', currency: 'credits', credits: 50 }
const last = <T,>(mock: { mock: { calls: T[] } }) => mock.mock.calls[mock.mock.calls.length - 1]

function renderFlow(providerType = ProviderType.INJECTED, current = listing) {
  const onClose = vi.fn()
  render(
    <UpdatePriceFlow
      item={item}
      collection={collection}
      listing={current}
      session={makeSession(providerType)}
      onClose={onClose}
    />,
    { wrapper: Providers }
  )
  return { onClose }
}

async function submitPrice(credits: string) {
  await userEvent.clear(screen.getByTestId('update-price-input'))
  await userEvent.type(screen.getByTestId('update-price-input'), credits)
  await userEvent.click(screen.getByTestId('update-price-submit'))
}

beforeEach(() => {
  update.mutate.mockReset()
  sell.mutate.mockReset()
  enable.mutate.mockReset()
  salesEnabled = true
})

describe('UpdatePriceFlow', () => {
  it('only accepts a new whole-credit price and shows the remaining supply', async () => {
    renderFlow()
    expect(screen.getByTestId('sell-item-availability')).toHaveTextContent('90 / 100 available')
    const submit = screen.getByTestId('update-price-submit')
    expect(submit).toBeDisabled()
    await userEvent.type(screen.getByTestId('update-price-input'), '50')
    expect(submit).toBeDisabled()
    await userEvent.clear(screen.getByTestId('update-price-input'))
    await userEvent.type(screen.getByTestId('update-price-input'), '8a0')
    expect(screen.getByTestId('update-price-usd')).toHaveTextContent('$8.00')
    expect(submit).toBeEnabled()
    await userEvent.type(screen.getByTestId('update-price-input'), '000000000000')
    expect(screen.getByTestId('update-price-input')).toHaveValue('800000000000')
  })

  it("opens in the listing's currency and lets the creator switch it, refusing the same price", async () => {
    renderFlow(ProviderType.INJECTED, { itemId: '3', tradeId: 'trade-1', currency: 'mana', manaWei: 5n * 10n ** 18n })
    const input = screen.getByTestId('update-price-input')
    expect(input).toHaveAttribute('data-currency', 'mana')
    await userEvent.type(input, '5')
    expect(screen.getByTestId('update-price-submit')).toBeDisabled()
    await userEvent.type(input, '.5')
    expect(screen.getByTestId('update-price-submit')).toBeEnabled()

    await userEvent.click(screen.getByTestId('update-price-currency'))
    await userEvent.click(screen.getByTestId('update-price-currency-option-credits'))
    expect(input).toHaveValue('')
    await userEvent.type(input, '50')
    await userEvent.click(screen.getByTestId('update-price-submit'))
    expect(last(update.mutate)[0]).toMatchObject({ price: { kind: 'credits', credits: 50 } })
  })

  it('walks a web3 wallet through both signatures, then stores the new order', async () => {
    const { onClose } = renderFlow()
    await submitPrice('80')
    expect(last(update.mutate)[0]).toMatchObject({ price: { kind: 'credits', credits: 80 } })
    expect(screen.getByTestId('update-price-signing-step-1')).toHaveAttribute('data-state', 'current')
    expect(screen.getByTestId('update-price-signing-label')).toHaveTextContent(/removal of your current price/i)
    expect(screen.getByTestId('update-price-signing-cancel')).toBeInTheDocument()

    await act(async () => last(update.mutate)[0].onSigned?.('cancel'))
    expect(screen.getByTestId('update-price-signing-step-1')).toHaveAttribute('data-state', 'done')
    expect(screen.getByTestId('update-price-signing-step-2')).toHaveAttribute('data-state', 'current')
    expect(screen.getByTestId('update-price-signing-label')).toHaveTextContent(/confirm your new price/i)
    expect(screen.queryByTestId('update-price-signing-cancel')).not.toBeInTheDocument()

    await act(async () => last(update.mutate)[0].onSigned?.('sign'))
    expect(screen.getByTestId('update-price-pending-label')).toHaveTextContent(/updating price/i)

    await act(async () => last(update.mutate)[1].onSuccess?.({}))
    expect(screen.getByTestId('success-modal-description')).toHaveTextContent(/new price/i)
    await userEvent.click(screen.getByTestId('success-modal-done'))
    expect(onClose).toHaveBeenCalled()
  })

  it('backs out of the first prompt to the form, and re-lists directly on retry once the old order is gone', async () => {
    renderFlow()
    await submitPrice('80')
    await userEvent.click(screen.getByTestId('update-price-signing-cancel'))
    expect(screen.getByTestId('update-price-input')).toHaveValue('80')

    await userEvent.click(screen.getByTestId('update-price-submit'))
    await act(async () => last(update.mutate)[0].onSigned?.('cancel'))
    await act(async () => last(update.mutate)[0].onCancelled?.({ beneficiary: ADDRESS, expiresAt: NO_EXPIRATION }))
    await act(async () => last(update.mutate)[1].onError?.({ code: 4001, message: 'User rejected' }))
    // The listing is already cancelled: even a dismissed prompt is reported.
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/update the price/i)

    await userEvent.click(screen.getByTestId('sale-error-retry'))
    await userEvent.click(screen.getByTestId('update-price-submit'))
    expect(update.mutate).toHaveBeenCalledTimes(2)
    expect(last(sell.mutate)[0]).toMatchObject({
      price: { kind: 'credits', credits: 80 },
      beneficiary: ADDRESS,
      expiresAt: NO_EXPIRATION
    })
    expect(screen.getByTestId('update-price-signing-step-2')).toHaveAttribute('data-state', 'current')
    await act(async () => last(sell.mutate)[1].onSuccess?.({}))
    expect(screen.getByTestId('success-modal-title')).toBeInTheDocument()
  })

  it('shows only "Updating price" to a social-login creator', async () => {
    renderFlow(ProviderType.MAGIC)
    await submitPrice('80')
    expect(screen.getByTestId('update-price-pending-label')).toHaveTextContent(/updating price/i)
    expect(screen.queryByTestId('update-price-pending-cancel')).not.toBeInTheDocument()
    expect(screen.queryByTestId('update-price-signing')).not.toBeInTheDocument()
  })

  describe('when re-pricing a collection the newest marketplace cannot mint yet', () => {
    beforeEach(() => {
      salesEnabled = false
      renderFlow()
    })

    it('should explain that the marketplace was upgraded', () => {
      expect(screen.getByTestId('enable-sales-modal-description')).toHaveTextContent(/marketplace was upgraded/i)
    })
  })

  describe('when the newest marketplace cannot mint the collection yet', () => {
    beforeEach(async () => {
      salesEnabled = false
      renderFlow()
      await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    })

    it('should ask to enable sales before showing the price form', () => {
      expect(enable.mutate).toHaveBeenCalledTimes(1)
    })

    it('should not touch the current listing until sales are enabled', () => {
      expect([update.mutate.mock.calls.length, screen.queryByTestId('update-price-input')]).toEqual([0, null])
    })

    describe('and the enable transaction succeeds', () => {
      beforeEach(() => {
        act(() => last(enable.mutate)[1].onSuccess?.(collection))
      })

      it('should show the price form', () => {
        expect(screen.getByTestId('update-price-input')).toBeInTheDocument()
      })
    })

    describe('and the enable transaction fails', () => {
      beforeEach(async () => {
        await act(async () => last(enable.mutate)[1].onError?.(new Error('reverted')))
      })

      it('should report that sales could not be enabled', () => {
        expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/enable sales/i)
      })

      describe('and the creator retries', () => {
        beforeEach(async () => {
          await userEvent.click(screen.getByTestId('sale-error-retry'))
        })

        it('should go back to the Enable Sales dialog instead of the price form', () => {
          expect([screen.getByTestId('enable-sales-modal'), screen.queryByTestId('update-price-input')]).toEqual([
            expect.anything(),
            null
          ])
        })
      })
    })

    describe('and the creator rejects the wallet prompt', () => {
      beforeEach(async () => {
        await act(async () => last(enable.mutate)[1].onError?.({ code: 4001, message: 'User rejected' }))
      })

      it('should go back to the Enable Sales dialog', () => {
        expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()
      })
    })

    describe('and the creator backs out of the wallet prompt', () => {
      beforeEach(async () => {
        await userEvent.click(screen.getByTestId('enable-sales-pending-cancel'))
      })

      it('should go back to the Enable Sales dialog', () => {
        expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()
      })

      describe('and the abandoned transaction goes through later', () => {
        beforeEach(async () => {
          await act(async () => last(enable.mutate)[1].onSuccess?.(collection))
        })

        it('should ignore it and stay on the Enable Sales dialog', () => {
          expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()
        })

        describe('and the creator enables sales again', () => {
          beforeEach(async () => {
            await userEvent.click(screen.getByTestId('enable-sales-confirm'))
          })

          it('should show the price form without sending a second transaction', () => {
            expect([enable.mutate.mock.calls.length, screen.queryByTestId('update-price-input')]).toEqual([
              1,
              expect.anything()
            ])
          })
        })
      })

      describe('and the creator enables sales again while that transaction is still unsettled', () => {
        beforeEach(async () => {
          await userEvent.click(screen.getByTestId('enable-sales-confirm'))
        })

        it('should wait for the transaction already sent instead of sending a second one', () => {
          expect([enable.mutate.mock.calls.length, screen.queryByTestId('enable-sales-pending')]).toEqual([
            1,
            expect.anything()
          ])
        })

        describe('and that transaction goes through', () => {
          beforeEach(async () => {
            await act(async () => last(enable.mutate)[1].onSuccess?.(collection))
          })

          it('should show the price form', () => {
            expect(screen.getByTestId('update-price-input')).toBeInTheDocument()
          })
        })
      })
    })
  })
})
