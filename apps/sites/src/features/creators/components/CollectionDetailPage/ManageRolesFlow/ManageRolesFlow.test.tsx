import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProviderType } from '@dcl/schemas'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { type Collection } from '~/lib/collections'
import { type RoleKind } from '~/lib/collectionRoles'
import { SellItemError } from '~/lib/sales'
import { WrongAccountError } from '~/lib/walletErrors'
import { ManageRolesFlow } from './ManageRolesFlow'
import { ADDRESS, FRIEND, Providers, collection, makeSession } from '../SellItemFlow/testUtils'

type Callbacks = { onSuccess?: (result: unknown) => void; onError?: (error: unknown) => void }
type Variables = { kind: RoleKind; current: string[]; next: string[]; onSigned?: () => void }

const save = { mutate: vi.fn<(variables: Variables, callbacks: Callbacks) => void>(), isPending: false }
vi.mock('~/hooks/useCollectionRoles', async importOriginal => ({
  ...(await importOriginal<typeof import('~/hooks/useCollectionRoles')>()),
  useSetCollectionRoles: () => save
}))
vi.mock('~/hooks/useSales', () => ({
  useFriends: () => ({
    data: [{ address: FRIEND, name: 'Ana', hasClaimedName: true, avatarUrl: '' }],
    isLoading: false
  })
}))
vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))

const MINTER = '0x00000000000000000000000000000000000000dd'
const MANAGER = '0x00000000000000000000000000000000000000ee'
const MARKETPLACE = getContract(ContractName.OffChainMarketplaceV2, 80002).address
const withRoles: Collection = { ...collection, minters: [MINTER.toUpperCase(), MARKETPLACE], managers: [MANAGER] }

function renderFlow(kind: RoleKind = 'senders', providerType = ProviderType.INJECTED, target = withRoles) {
  const onClose = vi.fn()
  render(<ManageRolesFlow collection={target} kind={kind} session={makeSession(providerType)} onClose={onClose} />, {
    wrapper: Providers
  })
  return { onClose }
}

const short = (address: string) => `${address.slice(0, 6)}…${address.slice(-4)}`
const last = <T,>(mock: { mock: { calls: T[] } }) => mock.mock.calls[mock.mock.calls.length - 1]

beforeEach(() => {
  save.mutate.mockReset()
  save.isPending = false
})

describe('ManageRolesFlow', () => {
  it('lists the senders without the Shop contract, and only enables saving once the list changes', async () => {
    renderFlow()
    expect(screen.getByTestId('manage-roles-modal')).toHaveTextContent(/senders/i)
    expect(screen.getAllByTestId('role-row')).toHaveLength(1)
    expect(screen.getByTestId('role-1-selected')).toHaveTextContent(short(MINTER))
    expect(screen.queryByTestId('role-new-input')).not.toBeInTheDocument()
    expect(screen.getByTestId('role-save')).toBeDisabled()

    await userEvent.click(screen.getByTestId('role-add'))
    await userEvent.type(screen.getByTestId('role-new-input'), FRIEND)
    expect(screen.getAllByTestId('role-row')).toHaveLength(2)
    expect(screen.getByTestId('role-2-name')).toHaveTextContent('Ana')
    expect(screen.getByTestId('role-save')).toBeEnabled()
  })

  it('lets the owner add themselves, so the role survives a transfer of ownership', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('role-add'))
    await userEvent.type(screen.getByTestId('role-new-input'), ADDRESS)
    expect(screen.queryByTestId('role-new-error')).not.toBeInTheDocument()
    expect(screen.getAllByTestId('role-row')).toHaveLength(2)
    await userEvent.click(screen.getByTestId('role-save'))
    expect(last(save.mutate)[0].next).toEqual([MINTER, ADDRESS.toLowerCase()])
  })

  it('refuses a repeated address and the Shop contract with an inline reason', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('role-add'))
    const input = screen.getByTestId('role-new-input')
    await userEvent.type(input, MINTER)
    expect(screen.getByTestId('role-new-error')).toHaveTextContent(/already/i)
    await userEvent.clear(input)
    await userEvent.type(input, MARKETPLACE)
    expect(screen.getByTestId('role-new-error')).toHaveTextContent(/shop/i)
    expect(screen.getAllByTestId('role-row')).toHaveLength(1)
    expect(screen.getByTestId('role-save')).toBeDisabled()
  })

  it('removes a sender from the draft straight away, since nothing changes until saving', async () => {
    renderFlow()
    await userEvent.click(screen.getByTestId('role-1-clear'))
    expect(screen.queryByTestId('role-row')).not.toBeInTheDocument()
    // The empty list shows the picker right away.
    expect(screen.getByTestId('role-new-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-save')).toBeEnabled()
  })

  it('closes straight away when nothing changed, but asks to discard unsaved changes', async () => {
    const { onClose } = renderFlow()
    await userEvent.click(screen.getByTestId('role-cancel'))
    expect(onClose).toHaveBeenCalledTimes(1)

    await userEvent.click(screen.getByTestId('role-add'))
    await userEvent.type(screen.getByTestId('role-new-input'), FRIEND)
    await userEvent.keyboard('{Escape}')
    expect(screen.getByTestId('discard-roles-modal-title')).toHaveTextContent(/discard/i)
    await userEvent.click(screen.getByTestId('discard-roles-keep'))
    expect(screen.queryByTestId('discard-roles-modal')).not.toBeInTheDocument()
    expect(screen.getAllByTestId('role-row')).toHaveLength(2)
    expect(onClose).toHaveBeenCalledTimes(1)

    await userEvent.click(screen.getByTestId('manage-roles-modal-close'))
    await userEvent.click(screen.getByTestId('discard-roles-leave'))
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('saves the diff in one transaction, walking a web3 wallet through the prompt, then celebrates', async () => {
    const { onClose } = renderFlow()
    await userEvent.click(screen.getByTestId('role-1-clear'))
    await userEvent.type(screen.getByTestId('role-new-input'), FRIEND)
    await userEvent.click(screen.getByTestId('role-save'))

    expect(screen.getByTestId('manage-roles-pending-label')).toHaveTextContent(/confirm in your wallet/i)
    const [variables, callbacks] = last(save.mutate)
    expect(variables).toMatchObject({ kind: 'senders', current: [MINTER], next: [FRIEND] })

    // Backing out of the prompt returns to the draft, untouched.
    await userEvent.click(screen.getByTestId('manage-roles-pending-cancel'))
    expect(screen.getByTestId('role-1-selected')).toHaveTextContent(short(FRIEND))
    await act(async () => callbacks.onSuccess?.(undefined))
    expect(screen.queryByTestId('success-modal-modal')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('role-save'))
    const [again, done] = last(save.mutate)
    await act(async () => again.onSigned?.())
    expect(screen.getByTestId('manage-roles-pending-label')).toHaveTextContent(/saving changes/i)
    expect(screen.queryByTestId('manage-roles-pending-cancel')).not.toBeInTheDocument()
    await act(async () => done.onSuccess?.(undefined))
    expect(screen.getByTestId('success-modal-title')).toHaveTextContent(/senders updated/i)
    await userEvent.click(screen.getByTestId('success-modal-done'))
    expect(onClose).toHaveBeenCalled()
  })

  it('keeps a custodial wallet in the dialog with the button spinning', async () => {
    renderFlow('collaborators', ProviderType.MAGIC)
    expect(screen.getByTestId('role-1-selected')).toHaveTextContent(short(MANAGER))
    await userEvent.click(screen.getByTestId('role-1-clear'))
    save.isPending = true
    await userEvent.click(screen.getByTestId('role-save'))
    expect(screen.queryByTestId('manage-roles-pending')).not.toBeInTheDocument()
    expect(last(save.mutate)[0]).toMatchObject({ kind: 'collaborators', current: [MANAGER], next: [] })
  })

  it('reports a failure and lets TRY AGAIN reopen the draft, while a dismissed prompt just returns to it', async () => {
    const { onClose } = renderFlow('collaborators')
    await userEvent.click(screen.getByTestId('role-add'))
    await userEvent.type(screen.getByTestId('role-new-input'), FRIEND)
    await userEvent.click(screen.getByTestId('role-save'))
    await act(async () => last(save.mutate)[1].onError?.(new SellItemError('rejected')))
    expect(screen.getAllByTestId('role-row')).toHaveLength(2)

    await userEvent.click(screen.getByTestId('role-save'))
    await act(async () => last(save.mutate)[1].onError?.(new Error('boom')))
    expect(screen.getByTestId('sale-error-modal-title')).toHaveTextContent(/collaborators/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getAllByTestId('role-row')).toHaveLength(2)
    expect(onClose).not.toHaveBeenCalled()
  })

  it('asks to switch the wallet back when it sits on another account, keeping the draft for a retry', async () => {
    renderFlow('collaborators')
    await userEvent.click(screen.getByTestId('role-add'))
    await userEvent.type(screen.getByTestId('role-new-input'), FRIEND)
    await userEvent.click(screen.getByTestId('role-save'))
    await act(async () => last(save.mutate)[1].onError?.(new WrongAccountError(ADDRESS, FRIEND)))
    expect(screen.getByTestId('sale-error-modal-description')).toHaveTextContent(/switch it back to the account/i)
    await userEvent.click(screen.getByTestId('sale-error-retry'))
    expect(screen.getAllByTestId('role-row')).toHaveLength(2)
  })
})
