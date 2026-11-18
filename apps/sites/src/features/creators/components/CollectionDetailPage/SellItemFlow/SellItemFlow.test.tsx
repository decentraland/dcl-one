import { describe, it, expect, vi, beforeEach } from 'vitest'
import { FeatureFlag } from '~/lib/featureFlags'
import { setFeatureFlags } from '~/test/featureFlags'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProviderType } from '@dcl/schemas'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { getMaticChainId } from '~/lib/publishCollection'
import { SellItemError } from '~/lib/sales'
import { SellItemFlow } from './SellItemFlow'
import { Providers, collection, item, makeSession } from './testUtils'

vi.mock('~/lib/featureFlags', async () => {
  const actual = await vi.importActual<typeof import('~/lib/featureFlags')>('~/lib/featureFlags')
  const mock = await import('~/test/featureFlags')
  return { ...actual, getIsFeatureEnabled: mock.getIsFeatureEnabled }
})

beforeEach(() => setFeatureFlags(FeatureFlag.OFFCHAIN_PUBLIC_ITEM_ORDERS, FeatureFlag.CREDITS_PRIMARY_LISTINGS))

type Callbacks = { onSuccess?: (result: unknown) => void; onError?: (error: unknown) => void }
type Variables = { onSigned?: () => void }

// Controllable stand-ins for the mutations: tests drive the wallet prompt, its signature and its outcome.
const enable = { mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(), isPending: false }
const sell = { mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(), isPending: false }
let salesEnabled = false

vi.mock('~/hooks/useSales', () => ({
  useSalesEnabled: () => salesEnabled,
  useEnableSales: () => enable,
  useSellItem: () => sell,
  useFriends: () => ({ data: [], isLoading: false }),
  useManaUsdRate: () => ({ data: undefined })
}))
vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))

function renderFlow(providerType = ProviderType.INJECTED, current = collection) {
  const onClose = vi.fn()
  const view = render(
    <SellItemFlow item={item} collection={current} session={makeSession(providerType)} onClose={onClose} />,
    { wrapper: Providers }
  )
  return { onClose, view }
}

const last = <T,>(mock: { mock: { calls: T[] } }) => mock.mock.calls[mock.mock.calls.length - 1]

async function fillAndSubmit(credits = '50') {
  await userEvent.type(screen.getByTestId('sell-price-input'), credits)
  await userEvent.click(screen.getByTestId('sell-submit'))
}

beforeEach(() => {
  enable.mutate.mockReset()
  sell.mutate.mockReset()
  enable.isPending = false
  sell.isPending = false
  salesEnabled = false
})

describe('SellItemFlow', () => {
  it('asks to enable sales first, walks a web3 wallet through the prompt and the mining, then opens the form', async () => {
    renderFlow()
    expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    expect(screen.getByTestId('enable-sales-pending-label')).toHaveTextContent(/confirm in your wallet/i)
    expect(screen.getByTestId('enable-sales-pending-cancel')).toBeInTheDocument()

    const [variables, callbacks] = last(enable.mutate)
    await act(async () => variables.onSigned?.())
    expect(screen.getByTestId('enable-sales-pending-label')).toHaveTextContent(/enabling sales/i)
    expect(screen.queryByTestId('enable-sales-pending-cancel')).not.toBeInTheDocument()

    await act(async () => callbacks.onSuccess?.(collection))
    expect(screen.getByTestId('sell-item-modal')).toBeInTheDocument()
  })

  it('lets the creator back out of the wallet prompt and ignores that attempt afterwards', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    await userEvent.click(screen.getByTestId('enable-sales-pending-cancel'))
    expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()

    await act(async () => last(enable.mutate)[1].onSuccess?.(collection))
    expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()
  })

  describe('when the creator backs out of the wallet prompt and signs the transaction anyway', () => {
    beforeEach(async () => {
      renderFlow()
      await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      await userEvent.click(screen.getByTestId('enable-sales-pending-cancel'))
      await act(async () => last(enable.mutate)[0].onSigned?.())
    })

    describe('and enables sales again before it is mined', () => {
      beforeEach(async () => {
        await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      })

      it('should show the mining status of the transaction already sent instead of sending a second one', () => {
        expect([enable.mutate.mock.calls.length, screen.getByTestId('enable-sales-pending-label')]).toEqual([
          1,
          expect.objectContaining({ textContent: expect.stringMatching(/enabling sales/i) })
        ])
      })

      describe('and it is mined', () => {
        beforeEach(async () => {
          await act(async () => last(enable.mutate)[1].onSuccess?.(collection))
        })

        it('should open the form', () => {
          expect(screen.getByTestId('sell-item-modal')).toBeInTheDocument()
        })
      })
    })

    describe('and it is mined before the creator enables sales again', () => {
      beforeEach(async () => {
        await act(async () => last(enable.mutate)[1].onSuccess?.(collection))
        await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      })

      it('should open the form without sending a second transaction', () => {
        expect([enable.mutate.mock.calls.length, screen.queryByTestId('sell-item-modal')]).toEqual([
          1,
          expect.anything()
        ])
      })
    })

    describe('and it fails before the creator enables sales again', () => {
      beforeEach(async () => {
        await act(async () => last(enable.mutate)[1].onError?.(new Error('reverted')))
        await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      })

      it('should send a new transaction', () => {
        expect(enable.mutate).toHaveBeenCalledTimes(2)
      })
    })
  })

  describe('when the creator backs out of a wallet prompt that never settles, twice', () => {
    beforeEach(async () => {
      renderFlow()
      await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      await userEvent.click(screen.getByTestId('enable-sales-pending-cancel'))
      await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      await userEvent.click(screen.getByTestId('enable-sales-pending-cancel'))
      await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    })

    it('should give up on that prompt and send a new transaction', () => {
      expect(enable.mutate).toHaveBeenCalledTimes(2)
    })

    describe('and the abandoned prompt fails after the new one was sent', () => {
      beforeEach(async () => {
        await act(async () => enable.mutate.mock.calls[0][1].onError?.(new Error('closed')))
        await userEvent.click(screen.getByTestId('enable-sales-pending-cancel'))
        await userEvent.click(screen.getByTestId('enable-sales-confirm'))
      })

      it('should keep resuming the new transaction instead of sending a third one', () => {
        expect(enable.mutate).toHaveBeenCalledTimes(2)
      })
    })
  })

  describe('when the collection has never been on sale', () => {
    beforeEach(() => {
      renderFlow()
    })

    it('should explain that its first sale needs sales enabled', () => {
      expect(screen.getByTestId('enable-sales-modal-description')).toHaveTextContent(/first item/i)
    })
  })

  describe('when the collection already sells on an older marketplace', () => {
    beforeEach(() => {
      const older = getContract(ContractName.OffChainMarketplaceV2, getMaticChainId()).address
      renderFlow(ProviderType.INJECTED, { ...collection, minters: [older] })
    })

    it('should explain that the marketplace was upgraded', () => {
      expect(screen.getByTestId('enable-sales-modal-description')).toHaveTextContent(/marketplace was upgraded/i)
    })
  })

  it('keeps a social-login creator on the Enable Sales dialog with a spinning button', async () => {
    const { view } = renderFlow(ProviderType.MAGIC)
    await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    enable.isPending = true
    view.rerender(
      <SellItemFlow item={item} collection={collection} session={makeSession(ProviderType.MAGIC)} onClose={vi.fn()} />
    )
    expect(screen.getByTestId('enable-sales-confirm')).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByTestId('enable-sales-pending')).not.toBeInTheDocument()
  })

  it('reports a failed enable with a retry back to the dialog, and treats a rejection as a change of mind', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    await act(async () => last(enable.mutate)[1].onError?.({ code: 4001, message: 'User rejected' }))
    expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('enable-sales-confirm'))
    await act(async () => last(enable.mutate)[1].onError?.(new Error('reverted')))
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/enable sales/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('enable-sales-modal')).toBeInTheDocument()
  })

  it('goes straight to the form once sales are enabled and celebrates a stored order', async () => {
    salesEnabled = true
    const { onClose } = renderFlow()
    await fillAndSubmit()
    expect(screen.getByTestId('sell-item-pending-label')).toHaveTextContent(/confirm in your wallet/i)

    const [variables, callbacks] = last(sell.mutate)
    expect(variables).toMatchObject({ item, collection, price: { kind: 'credits', credits: 50 } })
    await act(async () => variables.onSigned?.())
    expect(screen.getByTestId('sell-item-pending-label')).toHaveTextContent(/putting for sale/i)
    expect(screen.queryByTestId('sell-item-pending-cancel')).not.toBeInTheDocument()

    await act(async () => callbacks.onSuccess?.({}))
    expect(screen.getByTestId('success-modal-title')).toHaveTextContent(/on sale/i)
    await userEvent.click(screen.getByTestId('success-modal-done'))
    expect(onClose).toHaveBeenCalled()
  })

  it('brings the filled form back after a cancelled prompt or a failure, and closes on a sold-out item', async () => {
    salesEnabled = true
    const { onClose } = renderFlow()
    await fillAndSubmit('25')
    await userEvent.click(screen.getByTestId('sell-item-pending-cancel'))
    expect(screen.getByTestId('sell-price-input')).toHaveValue('25')

    await userEvent.click(screen.getByTestId('sell-submit'))
    await act(async () => last(sell.mutate)[1].onError?.(new Error('server down')))
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/put your item on sale/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('sell-price-input')).toHaveValue('25')

    await userEvent.click(screen.getByTestId('sell-submit'))
    await act(async () => last(sell.mutate)[1].onError?.(new SellItemError('sold_out')))
    expect(screen.getByTestId('sale-error-modal-description')).toHaveTextContent(/minted/i)
    expect(screen.queryByTestId('sale-error-retry')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('sale-error-cancel'))
    expect(onClose).toHaveBeenCalled()
  })

  it('spins the submit button for a social-login creator instead of showing the wallet status', async () => {
    salesEnabled = true
    const { view } = renderFlow(ProviderType.MAGIC)
    await fillAndSubmit()
    sell.isPending = true
    view.rerender(
      <SellItemFlow item={item} collection={collection} session={makeSession(ProviderType.MAGIC)} onClose={vi.fn()} />
    )
    expect(screen.queryByTestId('sell-item-pending')).not.toBeInTheDocument()
    expect(screen.getByTestId('sell-submit')).toHaveAttribute('aria-busy', 'true')
  })
})
