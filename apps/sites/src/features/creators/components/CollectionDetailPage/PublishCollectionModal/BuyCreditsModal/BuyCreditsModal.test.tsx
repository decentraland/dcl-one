import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BuyCreditsModal } from './BuyCreditsModal'
import { Providers } from '../../SellItemFlow/testUtils'

const CATALOGUE = {
  packs: [
    { id: 'pack_5', usd: 5.99, credits: 40, order: 1 },
    { id: 'pack_10', usd: 11.99, credits: 100, order: 2 },
    { id: 'pack_25', usd: 29.99, credits: 260, order: 3 },
    { id: 'pack_50', usd: 59.99, credits: 540, order: 4 }
  ]
}

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)
beforeEach(() => fetchMock.mockResolvedValue(new Response(JSON.stringify(CATALOGUE), { status: 200 })))

async function renderModal(props: Partial<React.ComponentProps<typeof BuyCreditsModal>> = {}) {
  const onBuy = vi.fn<React.ComponentProps<typeof BuyCreditsModal>['onBuy']>().mockResolvedValue()
  const onCancel = vi.fn()
  render(<BuyCreditsModal balance={60} shortfall={240} onCancel={onCancel} onBuy={onBuy} {...props} />, {
    wrapper: Providers
  })
  await waitFor(() => expect(screen.queryAllByTestId('credit-pack-skeleton')).toHaveLength(0))
  return { onBuy, onCancel }
}

describe('BuyCreditsModal', () => {
  it('preselects and badges the cheapest pack that covers the shortfall, with its total', async () => {
    await renderModal()
    expect(screen.getByTestId('credit-pack-pack_25')).toHaveAttribute('data-selected')
    expect(screen.getByTestId('credit-pack-pack_25')).toContainElement(screen.getByTestId('credit-pack-recommended'))
    expect(screen.getByTestId('buy-credits-total-usd')).toHaveTextContent('$29.99')
    expect(screen.getByTestId('buy-credits-submit')).toHaveTextContent('Buy 260 Credits')
    expect(screen.getByTestId('buy-credits-balance')).toHaveTextContent('60')
  })

  it('is a plain one-pack picker while a single pack can cover the shortfall', async () => {
    await renderModal()
    expect(screen.queryByTestId('credit-pack-pack_25-increase')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('credit-pack-pack_50'))
    expect(screen.getByTestId('credit-pack-pack_50')).toHaveAttribute('data-selected')
    expect(screen.getByTestId('credit-pack-pack_25')).not.toHaveAttribute('data-selected')
    expect(screen.getByTestId('buy-credits-submit')).toHaveTextContent('Buy 540 Credits')
    expect(screen.getByTestId('buy-credits-total-usd')).toHaveTextContent('$59.99')
  })

  it('preselects several of the largest pack when no single pack is enough', async () => {
    await renderModal({ shortfall: 1000 })
    expect(screen.getByTestId('credit-pack-pack_50-quantity')).toHaveTextContent('2')
    expect(screen.getByTestId('buy-credits-total-usd')).toHaveTextContent('$119.98')
    expect(screen.getByTestId('buy-credits-submit')).toHaveTextContent('Buy 1,080 Credits')
  })

  it('only ever buys copies of one pack: choosing another resets the first', async () => {
    await renderModal({ shortfall: 1000 })
    await userEvent.click(screen.getByTestId('credit-pack-pack_25-increase'))
    expect(screen.getByTestId('credit-pack-pack_25-quantity')).toHaveTextContent('1')
    expect(screen.getByTestId('credit-pack-pack_50-quantity')).toHaveTextContent('0')
    expect(screen.getByTestId('credit-pack-pack_50')).not.toHaveAttribute('data-selected')

    await userEvent.click(screen.getByTestId('credit-pack-pack_25-increase'))
    expect(screen.getByTestId('credit-pack-pack_25-quantity')).toHaveTextContent('2')
    expect(screen.getByTestId('buy-credits-submit')).toHaveTextContent('Buy 520 Credits')
    expect(screen.getByTestId('buy-credits-total-usd')).toHaveTextContent('$59.98')
  })

  it('disables buying once every pack is back to zero', async () => {
    await renderModal({ shortfall: 1000 })
    await userEvent.click(screen.getByTestId('credit-pack-pack_50-decrease'))
    await userEvent.click(screen.getByTestId('credit-pack-pack_50-decrease'))
    expect(screen.getByTestId('credit-pack-pack_50-quantity')).toHaveTextContent('0')
    expect(screen.getByTestId('buy-credits-submit')).toBeDisabled()
    expect(screen.getByTestId('credit-pack-pack_50-decrease')).toBeDisabled()
  })

  it('hands the chosen pack and quantity to the checkout', async () => {
    const { onBuy } = await renderModal({ shortfall: 1000 })
    await userEvent.click(screen.getByTestId('buy-credits-submit'))
    expect(onBuy).toHaveBeenCalledWith({ packId: 'pack_50', quantity: 2 }, { credits: 1080, usd: 119.98 })
  })

  it('keeps the dialog open with an error when the checkout cannot start', async () => {
    const { onBuy } = await renderModal()
    onBuy.mockRejectedValue(new Error('down'))
    await userEvent.click(screen.getByTestId('buy-credits-submit'))
    expect(await screen.findByTestId('buy-credits-error')).toBeInTheDocument()
    expect(screen.getByTestId('buy-credits-submit')).toBeEnabled()
  })

  it('shows pack skeletons until the catalogue answers, and the bundled packs if it never does', async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 503 }))
    render(<BuyCreditsModal balance={60} shortfall={240} onCancel={vi.fn()} onBuy={vi.fn()} />, {
      wrapper: Providers
    })
    expect(screen.getAllByTestId('credit-pack-skeleton')).toHaveLength(4)
    expect(await screen.findByTestId('credit-pack-pack_25')).toHaveAttribute('data-selected')
    expect(screen.queryAllByTestId('credit-pack-skeleton')).toHaveLength(0)
  })

  it('can be dismissed', async () => {
    const { onCancel } = await renderModal()
    await userEvent.click(screen.getByTestId('buy-credits-cancel'))
    expect(onCancel).toHaveBeenCalled()
  })
})
