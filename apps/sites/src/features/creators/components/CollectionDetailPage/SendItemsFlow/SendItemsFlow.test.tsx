import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProviderType } from '@dcl/schemas'
import { type Item } from '~/lib/items'
import { type Transfer } from '~/lib/mint'
import { useNotifications } from '~/lib/notifications'
import { WrongAccountError } from '~/lib/walletErrors'
import { track } from '~/lib/analytics'
import { Toasts } from '~/components/Toasts'
import { SendItemsFlow } from './SendItemsFlow'
import { ADDRESS, FRIEND, Providers, collection, item, makeSession } from '../SellItemFlow/testUtils'

type Callbacks = { onSuccess?: (result: unknown) => void; onError?: (error: unknown) => void }
type Variables = { transfers: Transfer[]; onSigned?: () => void }

const send = { mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(), isPending: false }
vi.mock('~/hooks/useSales', () => ({
  useSendItems: () => send,
  useFriends: () => ({ data: [], isLoading: false })
}))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))

const OTHER = '0x00000000000000000000000000000000000000dd'
const THIRD = '0x00000000000000000000000000000000000000ee'
// mythic: 10 copies. hat has 90 left (legendary), scarce 2, soldOut none, draft isn't sendable.
const scarce: Item = { ...item, id: 'i2', name: 'Scarce', tokenId: '4', rarity: 'mythic', totalSupply: 8 }
const soldOut: Item = { ...item, id: 'i3', name: 'Gone', tokenId: '5', totalSupply: 100 }
const draft: Item = { ...item, id: 'i4', name: 'Draft', tokenId: undefined, isPublished: false }
const items = [item, scarce, soldOut, draft]

function renderFlow(providerType = ProviderType.INJECTED, list = items) {
  const onClose = vi.fn()
  const view = render(
    <>
      <SendItemsFlow collection={collection} items={list} session={makeSession(providerType)} onClose={onClose} />
      <Toasts />
    </>,
    { wrapper: Providers }
  )
  return { onClose, view }
}

const last = <T,>(mock: { mock: { calls: T[] } }) => mock.mock.calls[mock.mock.calls.length - 1]

function pressBack() {
  act(() => {
    window.dispatchEvent(new PopStateEvent('popstate'))
  })
}

function fireBeforeUnload() {
  const event = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(event)
  return event.defaultPrevented
}

async function addRecipient(transfer: number, address: string) {
  await userEvent.type(screen.getByTestId(`transfer-${transfer}-recipient-input`), address)
}

async function paste(transfer: number, text: string) {
  await userEvent.click(screen.getByTestId(`transfer-${transfer}-recipient-input`))
  await userEvent.paste(text)
}

const wallets = (count: number, from = 1) =>
  Array.from({ length: count }, (_, i) => `0x${String(from + i).padStart(40, '0')}`)

beforeEach(() => {
  send.mutate.mockReset()
  send.isPending = false
  useNotifications.setState({ toasts: [] })
})

describe('SendItemsFlow', () => {
  it('lists only sendable items, with sold-out ones disabled, and needs a recipient plus a copy to continue', async () => {
    renderFlow()
    expect(screen.queryByTestId('transfer-1-item-i4')).not.toBeInTheDocument()
    expect(screen.getByTestId('transfer-1-item-i3-check')).toBeDisabled()
    expect(screen.getByTestId('transfer-1-item-i3-stock')).toHaveTextContent(/sold out/i)
    expect(screen.getByTestId('transfer-1-item-i1-stock')).toHaveTextContent('90 / 100 available')
    expect(screen.getByTestId('send-continue')).toBeDisabled()

    await userEvent.click(screen.getByTestId('transfer-1-item-i1-check'))
    expect(screen.getByTestId('transfer-1-item-i1-amount')).toHaveValue('1')
    expect(screen.getByTestId('send-continue')).toBeDisabled()

    await addRecipient(1, FRIEND)
    expect(screen.getByTestId('transfer-1-recipient-selected')).toBeInTheDocument()
    expect(screen.getByTestId('send-continue')).toBeEnabled()
    expect(screen.getByTestId('send-total')).toHaveTextContent('1 / 100 items')
  })

  it('steps the copies within the live stock, unchecking at zero, and skips a repeated recipient with a note', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-item-i2-plus'))
    await userEvent.click(screen.getByTestId('transfer-1-item-i2-plus'))
    expect(screen.getByTestId('transfer-1-item-i2-amount')).toHaveValue('2')
    expect(screen.getByTestId('transfer-1-item-i2-stock')).toHaveTextContent('0 / 10 available')
    expect(screen.getByTestId('transfer-1-item-i2-plus')).toBeDisabled()

    await userEvent.click(screen.getByTestId('transfer-1-item-i2-minus'))
    await userEvent.click(screen.getByTestId('transfer-1-item-i2-minus'))
    expect(screen.getByTestId('transfer-1-item-i2-check')).not.toBeChecked()
    expect(screen.getByTestId('transfer-1-item-i2-minus')).toBeDisabled()

    // A second recipient halves what each may get of the scarce item: 2 copies → 1 each.
    await userEvent.click(screen.getByTestId('transfer-1-item-i2-plus'))
    await userEvent.click(screen.getByTestId('transfer-1-item-i2-plus'))
    await userEvent.click(screen.getByTestId('transfer-1-add-recipient'))
    await addRecipient(1, FRIEND)
    expect(screen.getAllByTestId('transfer-1-recipient-selected')).toHaveLength(1)
    expect(screen.queryByTestId('transfer-1-recipient-error')).not.toBeInTheDocument()
    expect(screen.getByTestId('transfer-1-recipient-input')).toHaveValue('')
    expect(useNotifications.getState().toasts).toEqual([
      expect.objectContaining({ type: 'info', message: expect.stringMatching(/already/i) })
    ])

    await addRecipient(1, OTHER)
    expect(screen.getAllByTestId('transfer-1-recipient-selected')).toHaveLength(2)
    expect(screen.getByTestId('transfer-1-item-i2-amount')).toHaveValue('1')
    expect(screen.getByTestId('send-total')).toHaveTextContent('2 / 100 items')
  })

  it('offers the creator their own wallet first and takes it out of the list once chosen', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('transfer-1-recipient-toggle'))
    const options = screen.getAllByTestId('transfer-1-recipient-option')
    expect(options).toHaveLength(1)
    expect(options[0]).toHaveTextContent('You')
    await userEvent.click(options[0])
    expect(screen.getByTestId('transfer-1-recipient-you')).toBeInTheDocument()
    expect(screen.getByTestId('transfer-1-recipient-selected')).toHaveTextContent('0x0000…00aa')

    // With no friends and "You" already in, nobody is left to pick: the next field is a plain input.
    await userEvent.click(screen.getByTestId('transfer-1-add-recipient'))
    expect(screen.queryByTestId('transfer-1-recipient-toggle')).not.toBeInTheDocument()
  })

  it('takes a typed count, digits only and clamped to the stock, and unchecks an emptied one', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    const amount = screen.getByTestId('transfer-1-item-i2-amount')
    await userEvent.type(amount, '9x9')
    expect(amount).toHaveValue('2')
    expect(screen.getByTestId('transfer-1-item-i2-check')).toBeChecked()

    await userEvent.clear(amount)
    expect(amount).toHaveValue('')
    await userEvent.tab()
    expect(amount).toHaveValue('0')
    expect(screen.getByTestId('transfer-1-item-i2-check')).not.toBeChecked()
  })

  it('caps a send at 100 copies across transfers and explains why', async () => {
    renderFlow(ProviderType.INJECTED, [{ ...item, rarity: 'epic' }])
    await addRecipient(1, FRIEND)
    const amount = screen.getByTestId('transfer-1-item-i1-amount')
    await userEvent.type(amount, '100')
    expect(screen.getByTestId('send-total')).toHaveTextContent('100 / 100 items')
    expect(screen.getByTestId('send-total')).not.toHaveAttribute('data-over')
    expect(screen.getByTestId('send-continue')).toBeEnabled()

    await userEvent.type(amount, '{backspace}{backspace}{backspace}101')
    expect(amount).toHaveValue('101')
    expect(screen.getByTestId('send-total')).toHaveAttribute('data-over')
    expect(screen.getByTestId('send-total')).toHaveTextContent(/up to 100/i)
    expect(screen.getByTestId('send-continue')).toBeDisabled()
  })

  it('takes a pasted sheet column as recipients, says how many joined, and clears them all at once', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-add-recipient'))
    await paste(1, `wallet\tname\n${OTHER}\tOne\n${THIRD}\tTwo\n${FRIEND}\tAgain\nnot-an-address\n`)
    expect(screen.getAllByTestId('transfer-1-recipient-selected')).toHaveLength(3)
    expect(screen.queryByTestId('transfer-1-recipient-input')).not.toBeInTheDocument()
    expect(screen.getByTestId('toast')).toHaveTextContent('2 recipients added, 1 skipped: invalid or already added.')
    expect(vi.mocked(track)).toHaveBeenCalledWith('Paste recipients', {
      collectionId: collection.id,
      added: 2,
      invalid: 0,
      repeated: 1,
      cut: 0
    })

    // Three recipients share the scarce item's 2 copies: none can get one.
    expect(screen.getByTestId('transfer-1-item-i2-check')).toBeDisabled()
    await userEvent.click(screen.getByTestId('transfer-1-clear-recipients'))
    expect(screen.queryByTestId('transfer-1-recipient-selected')).not.toBeInTheDocument()
    expect(screen.getByTestId('transfer-1-recipient-input')).toBeInTheDocument()
    expect(screen.queryByTestId('transfer-1-clear-recipients')).not.toBeInTheDocument()
    expect(screen.getByTestId('transfer-1-item-i2-check')).toBeEnabled()
  })

  it('drops a pasted list of repeats like a typed one, keeps an invalid one with the reason, and closes a transfer at the send cap', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-add-recipient'))
    // Only repeats: dropped like a typed one, with the info toast.
    await paste(1, `${FRIEND}, ${FRIEND.toUpperCase().replace('0X', '0x')}`)
    expect(screen.getByTestId('transfer-1-recipient-input')).toHaveValue('')
    expect(screen.getByTestId('toast')).toHaveAttribute('data-type', 'info')
    expect(screen.getByTestId('toast')).toHaveTextContent(/already in this transfer/i)
    expect(screen.getAllByTestId('transfer-1-recipient-selected')).toHaveLength(1)

    // A bad checksum among them is an invalid address, kept in the field.
    await paste(1, `0xD8dA6bf26964af9d7eed9e03e53415d37aa96045 ${FRIEND}`)
    expect(screen.getByTestId('transfer-1-recipient-error')).toHaveTextContent(/valid wallet address/i)
    expect(screen.getAllByTestId('transfer-1-recipient-selected')).toHaveLength(1)

    await userEvent.clear(screen.getByTestId('transfer-1-recipient-input'))
    await paste(1, wallets(120).join('\n'))
    expect(screen.getAllByTestId('transfer-1-recipient-selected')).toHaveLength(100)
    expect(screen.getAllByTestId('toast').slice(-1)[0]).toHaveTextContent(
      '99 recipients added. You can send up to 100 items at a time.'
    )

    // A full transfer takes no more recipients, and another transfer could send nothing.
    expect(screen.queryByTestId('transfer-1-add-recipient')).not.toBeInTheDocument()
    expect(screen.queryByTestId('transfer-1-recipient-input')).not.toBeInTheDocument()
    expect(screen.queryByTestId('send-add-transfer')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('transfer-1-clear-recipients'))
    expect(screen.getByTestId('transfer-1-recipient-input')).toBeInTheDocument()
    expect(screen.getByTestId('send-add-transfer')).toBeInTheDocument()
  })

  it('closes straight away with nothing entered, but asks before discarding a recipient or a copy', async () => {
    const { onClose } = renderFlow()
    await userEvent.click(screen.getByTestId('send-cancel'))
    expect(onClose).toHaveBeenCalledTimes(1)

    await addRecipient(1, OTHER)
    await userEvent.keyboard('{Escape}')
    expect(screen.getByTestId('discard-send-modal-title')).toHaveTextContent(/discard/i)
    await userEvent.click(screen.getByTestId('discard-send-keep'))
    expect(screen.queryByTestId('discard-send-modal')).not.toBeInTheDocument()
    expect(screen.getByTestId('transfer-1-recipient-selected')).toBeInTheDocument()
    expect(onClose).toHaveBeenCalledTimes(1)

    pressBack()
    await userEvent.click(screen.getByTestId('discard-send-keep'))
    expect(fireBeforeUnload()).toBe(true)

    await userEvent.click(screen.getByTestId('send-items-modal-close'))
    await userEvent.click(screen.getByTestId('discard-send-leave'))
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('swallows back while the wallet prompt is up, closes on back once sent or failed, and never asks later', async () => {
    const { onClose } = renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-item-i1-check'))
    await userEvent.click(screen.getByTestId('send-continue'))
    await userEvent.click(screen.getByTestId('send-submit'))

    pressBack()
    expect(screen.getByTestId('send-items-pending-label')).toBeInTheDocument()
    await act(async () => last(send.mutate)[1].onError?.({ code: 4001, message: 'User rejected' }))
    expect(screen.getByTestId('send-summary')).toBeInTheDocument()
    expect(screen.queryByTestId('discard-send-modal')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('send-submit'))
    await act(async () => last(send.mutate)[1].onError?.(new Error('reverted')))
    // The draft is kept for TRY AGAIN, so a reload still warns; back closes like the error dialog's Cancel.
    expect(fireBeforeUnload()).toBe(true)
    pressBack()
    expect(onClose).toHaveBeenCalledTimes(1)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.queryByTestId('discard-send-modal')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('send-submit'))
    await act(async () => last(send.mutate)[1].onSuccess?.(undefined))
    expect(screen.getByTestId('success-modal-title')).toBeInTheDocument()
    expect(fireBeforeUnload()).toBe(false)
    pressBack()
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('removes a middle transfer without disturbing the ones after it', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('send-add-transfer'))
    await addRecipient(2, OTHER)
    // Leave the second transfer mid-edit, with an open input for another recipient.
    await userEvent.click(screen.getByTestId('transfer-2-add-recipient'))
    await userEvent.click(screen.getByTestId('send-add-transfer'))
    await addRecipient(3, THIRD)
    await userEvent.click(screen.getByTestId('transfer-3-item-i1-check'))
    expect(screen.getByTestId('transfer-2-recipient-input')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('transfer-2-remove'))
    expect(screen.queryByTestId('transfer-3')).not.toBeInTheDocument()
    expect(screen.getByTestId('transfer-2-recipient-selected')).toHaveTextContent(
      `${THIRD.slice(0, 6)}…${THIRD.slice(-4)}`
    )
    expect(screen.getByTestId('transfer-2-item-i1-amount')).toHaveValue('1')
    expect(screen.queryByTestId('transfer-2-recipient-input')).not.toBeInTheDocument()
    expect(screen.getByTestId('send-total')).toHaveTextContent('1 / 100 items')
  })

  it('reviews what each wallet receives across transfers, walks a web3 wallet through the prompt and celebrates the send', async () => {
    const { onClose } = renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-item-i1-check'))
    await userEvent.click(screen.getByTestId('send-add-transfer'))
    await addRecipient(2, OTHER)
    await userEvent.click(screen.getByTestId('transfer-2-add-recipient'))
    await addRecipient(2, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-2-item-i2-plus'))
    // An extra, empty transfer is ignored rather than blocking the send.
    await userEvent.click(screen.getByTestId('send-add-transfer'))
    await userEvent.click(screen.getByTestId('send-continue'))

    // The friend is in both transfers: one card with both bundles merged.
    expect(screen.getByTestId('send-steps-2')).toHaveAttribute('data-state', 'current')
    expect(screen.getByTestId('send-summary')).toHaveTextContent('2 Recipients - 2 Items - 3 Copies')
    expect(screen.getAllByTestId('send-summary-recipient')).toHaveLength(2)
    expect(screen.getByTestId(`send-summary-${FRIEND}-i1`)).toHaveTextContent('1')
    expect(screen.getByTestId(`send-summary-${FRIEND}-i2`)).toHaveTextContent('1')
    expect(screen.getByTestId(`send-summary-${OTHER}-i2`)).toHaveTextContent('1')
    expect(screen.queryByTestId(`send-summary-${OTHER}-i1`)).not.toBeInTheDocument()

    // Back keeps everything and returns to the first step.
    await userEvent.click(screen.getByTestId('send-back'))
    expect(screen.getByTestId('send-steps-1')).toHaveAttribute('data-state', 'current')
    expect(screen.getByTestId('transfer-2-item-i2-amount')).toHaveValue('1')
    await userEvent.click(screen.getByTestId('send-continue'))

    await userEvent.click(screen.getByTestId('send-submit'))
    expect(screen.getByTestId('send-items-pending-label')).toHaveTextContent(/confirm in your wallet/i)
    const [variables, callbacks] = last(send.mutate)
    expect(variables.transfers).toMatchObject([
      { recipients: [FRIEND], amounts: { i1: 1 } },
      { recipients: [OTHER, FRIEND], amounts: { i2: 1 } },
      { recipients: [], amounts: {} }
    ])
    await act(async () => variables.onSigned?.())
    expect(screen.getByTestId('send-items-pending-label')).toHaveTextContent(/sending items/i)
    expect(screen.queryByTestId('send-items-pending-cancel')).not.toBeInTheDocument()

    await act(async () => callbacks.onSuccess?.(undefined))
    expect(screen.getByTestId('success-modal-title')).toHaveTextContent(/items sent/i)
    await userEvent.click(screen.getByTestId('success-modal-done'))
    expect(onClose).toHaveBeenCalled()
  })

  it('returns to the confirm step with everything kept after a cancelled prompt, a rejection or a retry', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-item-i1-check'))
    await userEvent.click(screen.getByTestId('send-continue'))

    await userEvent.click(screen.getByTestId('send-submit'))
    await userEvent.click(screen.getByTestId('send-items-pending-cancel'))
    expect(screen.getByTestId('send-summary')).toBeInTheDocument()
    await act(async () => last(send.mutate)[1].onSuccess?.(undefined))
    expect(screen.queryByTestId('success-modal-title')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('send-submit'))
    await act(async () => last(send.mutate)[1].onError?.({ code: 4001, message: 'User rejected' }))
    expect(screen.getByTestId('send-summary')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('send-submit'))
    await act(async () => last(send.mutate)[1].onError?.(new Error('reverted')))
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/send your items/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('send-summary')).toHaveTextContent('1 Recipient - 1 Item - 1 Copy')
  })

  it('asks to switch the wallet back when it sits on another account, keeping the send for a retry', async () => {
    renderFlow()
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-item-i1-check'))
    await userEvent.click(screen.getByTestId('send-continue'))
    await userEvent.click(screen.getByTestId('send-submit'))
    await act(async () => last(send.mutate)[1].onError?.(new WrongAccountError(ADDRESS, FRIEND)))
    expect(screen.getByTestId('sale-error-modal-description')).toHaveTextContent(/switch it back to the account/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getByTestId('send-summary')).toHaveTextContent('1 Recipient - 1 Item - 1 Copy')
  })

  it('keeps a social-login creator on the dialog with a spinning button', async () => {
    const { view } = renderFlow(ProviderType.MAGIC)
    await addRecipient(1, FRIEND)
    await userEvent.click(screen.getByTestId('transfer-1-item-i1-check'))
    await userEvent.click(screen.getByTestId('send-continue'))
    await userEvent.click(screen.getByTestId('send-submit'))
    send.isPending = true
    view.rerender(
      <SendItemsFlow
        collection={collection}
        items={items}
        session={makeSession(ProviderType.MAGIC)}
        onClose={vi.fn()}
      />
    )
    expect(screen.getByTestId('send-submit')).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByTestId('send-items-pending')).not.toBeInTheDocument()
  })
})
