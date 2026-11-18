import { describe, it, expect, vi, beforeEach } from 'vitest'
import { type ReactNode } from 'react'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { ItemType, type Item } from '~/lib/items'
import { LinkedPublishError } from '~/lib/publishLinkedItems'
import { type ThirdParty } from '~/lib/thirdParty'
import { type ThirdPartyPrice } from '~/lib/thirdPartyRegistry'
import { PublishLinkedItemsModal } from './PublishLinkedItemsModal'

const publishMutate = vi.fn()
const slotsRefetch = vi.fn()
let slotsData = { available: 5, registered: true }

vi.mock('~/hooks/useLinkedPublish', () => ({
  usePublishLinkedItems: () => ({ mutate: publishMutate, isPending: false }),
  useSignSlotsCheque: () => ({
    isPending: false,
    isError: false,
    mutate: (_variables: unknown, options: { onSuccess: (cheque: unknown) => void }) =>
      options.onSuccess({ signature: '0xsig', qty: 1, salt: '0xsalt' })
  }),
  useThirdPartyPrice: () => ({
    data: price,
    isError: false,
    refetch: () => Promise.resolve({ data: price, isError: false })
  }),
  useLinkedSlots: () => ({ data: slotsData, isError: false, refetch: slotsRefetch })
}))
vi.mock('~/hooks/useLinkedCollection', () => ({ useItemCurations: () => ({ data: new Map() }) }))
vi.mock('~/hooks/useCollection', () => ({
  useSaveCollection: () => ({ mutate: vi.fn(), isPending: false, error: null, reset: vi.fn() })
}))
vi.mock('~/hooks/useProfile', () => ({ useProfile: () => ({ data: { email: 'jane.doe@example.com' } }) }))
vi.mock('~/hooks/useBalances', () => ({ useManaBalance: () => ({ data: 10n ** 24n, isSuccess: true }) }))

const ETHER = 10n ** 18n
const price: ThirdPartyPrice = {
  itemSlotUsdWei: 100n * ETHER,
  itemSlotManaWei: 200n * ETHER,
  programmaticMinSlots: 200,
  usdPerManaWei: ETHER / 2n
}

const collection = {
  id: 'c-double-submit',
  name: 'Brand Hats',
  urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats',
  isPublished: false,
  linkedContractAddress: '0x1d9fb685c257e74f869ba302e260c0b68f5ebb37',
  linkedContractNetwork: 'amoy'
} as Collection

const thirdParty = {
  id: 'urn:decentraland:amoy:collections-thirdparty:brand',
  name: 'Brand',
  managers: [],
  published: true,
  isProgrammatic: false
} as unknown as ThirdParty

const item = {
  id: 'i1',
  name: 'Gold Hat',
  thumbnail: 'thumbnail.png',
  contents: {},
  type: ItemType.WEARABLE,
  data: { category: 'hat', representations: [] }
} as unknown as Item

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

async function goToPayment() {
  render(
    <PublishLinkedItemsModal
      collection={collection}
      thirdParty={thirdParty}
      session={{ address: '0xabc' } as unknown as Session}
      itemsToPublish={[item]}
      itemsToPush={[]}
      onClose={vi.fn()}
      onPublished={vi.fn()}
    />,
    { wrapper }
  )
  await userEvent.click(screen.getByTestId('publish-name-accept'))
  await userEvent.click(screen.getByTestId('publish-name-confirm'))
  await userEvent.click(screen.getByTestId('publish-linked-items-accept'))
  await userEvent.click(screen.getByTestId('publish-linked-items-confirm'))
  await userEvent.click(screen.getByTestId('publish-linked-terms-accept'))
}

describe('PublishLinkedItemsModal', () => {
  beforeEach(() => {
    publishMutate.mockReset()
    slotsRefetch.mockReset()
    slotsData = { available: 5, registered: true }
    localStorage.clear()
  })

  it('starts one paying run however many times Submit is clicked while the slots are re-read', async () => {
    let finishRead: (value: { data: typeof slotsData; isError: boolean }) => void = () => undefined
    slotsRefetch.mockReturnValue(new Promise(resolve => (finishRead = resolve)))
    await goToPayment()

    const submit = screen.getByTestId('publish-linked-submit')
    await userEvent.click(submit)
    await userEvent.click(submit)
    expect(await screen.findByTestId('publish-linked-running')).toBeInTheDocument()

    finishRead({ data: { available: 5, registered: true }, isError: false })
    await waitFor(() => expect(publishMutate).toHaveBeenCalledTimes(1))
    // Nothing to buy: no payment phase among the few the creator sees.
    expect(screen.getByTestId('publish-linked-running')).toHaveTextContent('Prepare')
    expect(screen.getByTestId('publish-linked-running')).not.toHaveTextContent('Pay')
    expect(publishMutate.mock.calls[0][0]).toMatchObject({ slots: { available: 5 } })
  })

  it('stops and shows the new fee when the re-read slots would cost more than the fee shown', async () => {
    slotsRefetch.mockResolvedValue({ data: { available: 0, registered: true }, isError: false })
    await goToPayment()
    expect(screen.getByTestId('publish-linked-summary-total')).toHaveTextContent('No fee')

    await userEvent.click(screen.getByTestId('publish-linked-submit'))

    expect(await screen.findByTestId('publish-linked-fee-changed')).toBeInTheDocument()
    expect(publishMutate).not.toHaveBeenCalled()
  })

  it('shows the fee in USD only, like the standard publish', async () => {
    slotsData = { available: 0, registered: true }
    await goToPayment()
    expect(screen.getByTestId('publish-linked-summary-total')).toHaveTextContent(/^\$100\.00$/)
  })

  it('does not pay on a stale slot count when the re-read fails', async () => {
    slotsRefetch.mockResolvedValue({ data: slotsData, isError: true })
    await goToPayment()

    await userEvent.click(screen.getByTestId('publish-linked-submit'))

    expect(await screen.findByTestId('publish-linked-fee-error')).toBeInTheDocument()
    expect(publishMutate).not.toHaveBeenCalled()
  })

  it('resumes an unfinished payment remembered from an earlier attempt instead of paying again', async () => {
    const payment = { txHash: '0xabc123', registration: false }
    localStorage.setItem(`linked-publish.pending.80002.0xabc.${collection.id}`, JSON.stringify({ payment }))
    // Still pending, so the fee reads as if it were never sent: the run confirms it before buying anything.
    slotsData = { available: 0, registered: true }
    slotsRefetch.mockResolvedValue({ data: slotsData, isError: false })
    await goToPayment()

    expect(screen.getByTestId('publish-linked-resuming')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('publish-linked-submit'))

    await waitFor(() => expect(publishMutate).toHaveBeenCalledTimes(1))
    expect(publishMutate.mock.calls[0][0]).toMatchObject({ paidTx: payment })
  })

  it('stores a payment the moment the run reports it, before the run ends', async () => {
    slotsRefetch.mockResolvedValue({ data: slotsData, isError: false })
    await goToPayment()
    await userEvent.click(screen.getByTestId('publish-linked-submit'))
    await waitFor(() => expect(publishMutate).toHaveBeenCalledTimes(1))

    const payment = { txHash: '0xdef456', registration: true }
    publishMutate.mock.calls[0][0].onPayment(payment)

    expect(JSON.parse(localStorage.getItem(`linked-publish.pending.80002.0xabc.${collection.id}`)!)).toEqual({
      payment
    })
  })

  it('after the push failed, retries only the push, without quoting or paying for the published items', async () => {
    slotsRefetch.mockResolvedValue({ data: slotsData, isError: false })
    const onPublished = vi.fn()
    render(
      <PublishLinkedItemsModal
        collection={collection}
        thirdParty={thirdParty}
        session={{ address: '0xabc' } as unknown as Session}
        itemsToPublish={[item]}
        itemsToPush={[{ ...item, id: 'i2' }]}
        onClose={vi.fn()}
        onPublished={onPublished}
      />,
      { wrapper }
    )
    await userEvent.click(screen.getByTestId('publish-name-accept'))
    await userEvent.click(screen.getByTestId('publish-name-confirm'))
    await userEvent.click(screen.getByTestId('publish-linked-items-accept'))
    await userEvent.click(screen.getByTestId('publish-linked-items-confirm'))
    await userEvent.click(screen.getByTestId('publish-linked-terms-accept'))
    await userEvent.click(screen.getByTestId('publish-linked-submit'))
    await waitFor(() => expect(publishMutate).toHaveBeenCalledTimes(1))

    const [, callbacks] = publishMutate.mock.calls[0] as [
      unknown,
      { onError: (error: unknown) => void; onSettled: () => void }
    ]
    act(() => {
      callbacks.onError(new LinkedPublishError('push', 'push_failed'))
      callbacks.onSettled()
    })
    expect(onPublished).toHaveBeenCalled()
    await userEvent.click(screen.getByTestId('publish-error-retry'))
    await userEvent.click(screen.getByTestId('publish-linked-submit'))

    await waitFor(() => expect(publishMutate).toHaveBeenCalledTimes(2))
    expect(publishMutate.mock.calls[1][0]).toMatchObject({ itemsToPublish: [] })
  })

  it('lets a creator who cancelled a never-confirming payment forget it and pay again', async () => {
    const key = `linked-publish.pending.80002.0xabc.${collection.id}`
    localStorage.setItem(key, JSON.stringify({ payment: { txHash: '0xabc123', registration: false } }))
    slotsRefetch.mockResolvedValue({ data: slotsData, isError: false })
    await goToPayment()
    await userEvent.click(screen.getByTestId('publish-linked-submit'))
    await waitFor(() => expect(publishMutate).toHaveBeenCalledTimes(1))

    const [, callbacks] = publishMutate.mock.calls[0] as [
      unknown,
      { onError: (error: unknown) => void; onSettled: () => void }
    ]
    act(() => {
      callbacks.onError(new LinkedPublishError('wait_tx', 'payment_pending'))
      callbacks.onSettled()
    })
    await userEvent.click(screen.getByTestId('publish-forget-payment'))

    expect(localStorage.getItem(key)).toBeNull()
    expect(screen.getByTestId('publish-linked-payment-step')).toBeInTheDocument()
    expect(screen.queryByTestId('publish-linked-resuming')).not.toBeInTheDocument()
  })
})
