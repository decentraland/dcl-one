import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProviderType } from '@dcl/schemas'
import { type ItemListing } from '~/lib/listings'
import { RemoveListingFlow } from './RemoveListingFlow'
import { Providers, collection, item, makeSession } from './testUtils'

type Callbacks = { onSuccess?: () => void; onError?: (error: unknown) => void }
type Variables = { onSigned?: () => void }
const remove = { mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(), isPending: false }
vi.mock('~/hooks/useSales', () => ({ useRemoveListing: () => remove }))

const listing: ItemListing = { itemId: '3', tradeId: 'trade-1', currency: 'credits', credits: 50 }
const last = () => remove.mutate.mock.calls[remove.mutate.mock.calls.length - 1]

function renderFlow(providerType = ProviderType.INJECTED) {
  const onClose = vi.fn()
  const view = render(
    <RemoveListingFlow
      item={item}
      collection={collection}
      listing={listing}
      session={makeSession(providerType)}
      onClose={onClose}
    />,
    { wrapper: Providers }
  )
  return { onClose, view }
}

beforeEach(() => {
  remove.mutate.mockReset()
  remove.isPending = false
})

describe('RemoveListingFlow', () => {
  it('asks for confirmation, walks a web3 wallet through the prompt and the mining, then celebrates', async () => {
    const { onClose } = renderFlow()
    expect(screen.getByTestId('remove-listing-modal-description')).toHaveTextContent(item.name)
    await userEvent.click(screen.getByTestId('remove-listing-confirm'))
    expect(screen.getByTestId('remove-listing-pending-label')).toHaveTextContent(/confirm in your wallet/i)
    expect(last()[0]).toMatchObject({ collection, listing })

    await act(async () => last()[0].onSigned?.())
    expect(screen.getByTestId('remove-listing-pending-label')).toHaveTextContent(/removing from sale/i)
    expect(screen.queryByTestId('remove-listing-pending-cancel')).not.toBeInTheDocument()

    await act(async () => last()[1].onSuccess?.())
    expect(screen.getByTestId('success-modal-title')).toHaveTextContent(/removed from sale/i)
    await userEvent.click(screen.getByTestId('success-modal-done'))
    expect(onClose).toHaveBeenCalled()
  })

  it('returns to the confirmation after a cancelled or rejected prompt, and reports other failures', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('remove-listing-confirm'))
    await userEvent.click(screen.getByTestId('remove-listing-pending-cancel'))
    expect(screen.getByTestId('remove-listing-modal')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('remove-listing-confirm'))
    await act(async () => last()[1].onError?.({ code: 4001, message: 'User rejected' }))
    expect(screen.getByTestId('remove-listing-modal')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('remove-listing-confirm'))
    await act(async () => last()[1].onError?.(new Error('reverted')))
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/remove/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('remove-listing-modal')).toBeInTheDocument()
  })

  it('keeps a social-login creator on the confirmation with a spinning button', async () => {
    const { view } = renderFlow(ProviderType.MAGIC)
    await userEvent.click(screen.getByTestId('remove-listing-confirm'))
    remove.isPending = true
    view.rerender(
      <RemoveListingFlow
        item={item}
        collection={collection}
        listing={listing}
        session={makeSession(ProviderType.MAGIC)}
        onClose={vi.fn()}
      />
    )
    expect(screen.getByTestId('remove-listing-confirm')).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByTestId('remove-listing-pending')).not.toBeInTheDocument()
  })
})
