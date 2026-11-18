import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProviderType } from '@dcl/schemas'
import { SellItemError } from '~/lib/sales'
import { WrongAccountError } from '~/lib/walletErrors'
import { TransferOwnershipFlow } from './TransferOwnershipFlow'
import { ADDRESS, FRIEND, Providers, collection, makeSession } from '../SellItemFlow/testUtils'

type Callbacks = { onSuccess?: (result: unknown) => void; onError?: (error: unknown) => void }
type Variables = { newOwner: string; onSigned?: () => void }

const transfer = {
  mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(),
  isPending: false,
  data: undefined as { owner: string } | undefined
}
vi.mock('~/hooks/useTransferCollectionOwnership', () => ({ useTransferCollectionOwnership: () => transfer }))
vi.mock('~/hooks/useSales', () => ({
  useFriends: () => ({
    data: [{ address: FRIEND, name: 'Ana', hasClaimedName: true, avatarUrl: '' }],
    isLoading: false
  })
}))
vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))

function renderFlow(providerType = ProviderType.INJECTED) {
  const onClose = vi.fn()
  const onDone = vi.fn()
  render(
    <TransferOwnershipFlow
      collection={collection}
      session={makeSession(providerType)}
      onDone={onDone}
      onClose={onClose}
    />,
    { wrapper: Providers }
  )
  return { onClose, onDone }
}

async function fillIn(name = collection.name) {
  await userEvent.type(screen.getByTestId('new-owner-input'), FRIEND)
  await userEvent.type(screen.getByTestId('transfer-confirmation-input'), name)
  await userEvent.click(screen.getByTestId('transfer-continue'))
}

const SIGNED = '0x00000000000000000000000000000000000000cc'

const last = <T,>(mock: { mock: { calls: T[] } }) => mock.mock.calls[mock.mock.calls.length - 1]

beforeEach(() => {
  transfer.mutate.mockReset()
  transfer.isPending = false
  transfer.data = undefined
})

describe('TransferOwnershipFlow', () => {
  it('moves on only once a new owner is picked and the collection name is typed, then shows the wallet again to confirm', async () => {
    renderFlow()
    expect(screen.getByTestId('transfer-ownership-modal')).toHaveTextContent(/transfer ownership/i)
    expect(screen.getByTestId('transfer-continue')).toBeDisabled()

    await userEvent.type(screen.getByTestId('new-owner-input'), FRIEND)
    expect(screen.getByTestId('new-owner-name')).toHaveTextContent('Ana')
    expect(screen.getByTestId('transfer-continue')).toBeDisabled()

    await userEvent.type(screen.getByTestId('transfer-confirmation-input'), 'Pirate Hat')
    expect(screen.getByTestId('transfer-continue')).toBeDisabled()
    await userEvent.type(screen.getByTestId('transfer-confirmation-input'), 's')
    expect(screen.getByTestId('transfer-continue')).toBeEnabled()

    await userEvent.click(screen.getByTestId('transfer-continue'))
    expect(screen.getByTestId('transfer-confirm-owner')).toHaveTextContent('Ana')
    expect(screen.getByTestId('transfer-confirm-owner')).toHaveTextContent(FRIEND)
    expect(screen.getByTestId('transfer-ownership-warning')).toHaveTextContent(/can’t undo/i)
    expect(transfer.mutate).not.toHaveBeenCalled()

    // Back keeps what was filled in.
    await userEvent.click(screen.getByTestId('transfer-back'))
    expect(screen.getByTestId('new-owner-name')).toHaveTextContent('Ana')
    expect(screen.getByTestId('transfer-confirmation-input')).toHaveValue(collection.name)
  })

  it("refuses the owner's own address with an inline reason", async () => {
    renderFlow()
    await userEvent.type(screen.getByTestId('new-owner-input'), ADDRESS)
    expect(screen.getByTestId('new-owner-error')).toHaveTextContent(/your own address/i)
    expect(screen.queryByTestId('new-owner-selected')).not.toBeInTheDocument()
  })

  it('closes straight away, even with something filled in: two fields are nothing to lose', async () => {
    const { onClose } = renderFlow()
    await userEvent.click(screen.getByTestId('transfer-cancel'))
    expect(onClose).toHaveBeenCalledTimes(1)

    await userEvent.type(screen.getByTestId('transfer-confirmation-input'), 'Pir')
    await userEvent.keyboard('{Escape}')
    expect(onClose).toHaveBeenCalledTimes(2)
    await userEvent.click(screen.getByTestId('transfer-ownership-modal-close'))
    expect(onClose).toHaveBeenCalledTimes(3)
  })

  it('walks a web3 wallet through the prompt with no way out, then celebrates and hands over on DONE', async () => {
    const { onClose, onDone } = renderFlow()
    await fillIn()
    await userEvent.click(screen.getByTestId('transfer-submit'))

    // The prompt can still sign an irreversible transfer, so the dialog offers no cancel until it settles.
    expect(screen.getByTestId('transfer-ownership-pending-label')).toHaveTextContent(/confirm in your wallet/i)
    expect(screen.queryByTestId('transfer-ownership-pending-cancel')).not.toBeInTheDocument()
    const [variables, callbacks] = last(transfer.mutate)
    expect(variables).toMatchObject({ newOwner: FRIEND })

    await act(async () => variables.onSigned?.())
    expect(screen.getByTestId('transfer-ownership-pending-label')).toHaveTextContent(/transferring ownership/i)
    transfer.data = { owner: SIGNED }
    await act(async () => callbacks.onSuccess?.(undefined))
    expect(screen.getByTestId('success-modal-title')).toHaveTextContent(/ownership transferred/i)
    // The address named is the one the transfer went to, as the server reports it.
    expect(screen.getByTestId('success-modal-description')).toHaveTextContent(/0x0000…00cc/)
    await userEvent.click(screen.getByTestId('success-modal-done'))
    expect(onDone).toHaveBeenCalledTimes(1)
    expect(onClose).not.toHaveBeenCalled()
  })

  it('keeps a custodial wallet in the dialog, locked, with the button spinning', async () => {
    const { onClose } = renderFlow(ProviderType.MAGIC)
    await fillIn()
    transfer.isPending = true
    await userEvent.click(screen.getByTestId('transfer-submit'))
    expect(screen.queryByTestId('transfer-ownership-pending')).not.toBeInTheDocument()
    expect(last(transfer.mutate)[0]).toMatchObject({ newOwner: FRIEND })
    await act(async () => last(transfer.mutate)[0].onSigned?.())
    expect(screen.getByTestId('transfer-back')).toBeDisabled()
    await userEvent.keyboard('{Escape}')
    expect(onClose).not.toHaveBeenCalled()
  })

  it('reports a failure and lets TRY AGAIN reopen the form, while a dismissed prompt just returns to it', async () => {
    const { onClose } = renderFlow()
    await fillIn()
    await userEvent.click(screen.getByTestId('transfer-submit'))
    await act(async () => last(transfer.mutate)[1].onError?.(new SellItemError('rejected')))
    expect(screen.getByTestId('transfer-confirm-owner')).toHaveTextContent('Ana')

    await userEvent.click(screen.getByTestId('transfer-submit'))
    await act(async () => last(transfer.mutate)[1].onError?.(new Error('boom')))
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/transfer the ownership/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('transfer-confirm-owner')).toHaveTextContent('Ana')
    expect(onClose).not.toHaveBeenCalled()
  })

  it('asks to switch the wallet back when it sits on another account, keeping the form for a retry', async () => {
    renderFlow()
    await fillIn()
    await userEvent.click(screen.getByTestId('transfer-submit'))
    await act(async () => last(transfer.mutate)[1].onError?.(new WrongAccountError(ADDRESS, FRIEND)))
    expect(screen.getByTestId('sale-error-modal-description')).toHaveTextContent(/switch it back to the account/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('transfer-confirm-owner')).toHaveTextContent('Ana')
  })
})
