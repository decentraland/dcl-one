import { describe, it, expect, vi, beforeEach } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { BuilderServerError } from '~/lib/builder'
import { CreateLinkedCollectionModal } from './CreateLinkedCollectionModal'

vi.mock('~/lib/linkedContract', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/linkedContract')>()),
  checkLinkedContract: vi.fn()
}))

import { checkLinkedContract } from '~/lib/linkedContract'

const CONTRACT = '0x1d9fb685c257e74f869ba302e260c0b68f5ebb37'
const checkMock = vi.mocked(checkLinkedContract)

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderModal(props: Partial<React.ComponentProps<typeof CreateLinkedCollectionModal>> = {}) {
  const onSubmit = vi.fn()
  const onClose = vi.fn()
  render(
    <CreateLinkedCollectionModal isPending={false} error={null} onSubmit={onSubmit} onClose={onClose} {...props} />,
    {
      wrapper
    }
  )
  return { onSubmit, onClose }
}

async function fillForm(address = CONTRACT) {
  await userEvent.type(screen.getByTestId('linked-collection-name-input'), 'Bored Hats')
  await userEvent.click(screen.getByTestId('linked-network'))
  await userEvent.click(within(screen.getByRole('listbox')).getByText('Amoy'))
  await userEvent.type(screen.getByTestId('linked-contract-address-input'), address)
}

beforeEach(() => {
  checkMock.mockReset()
})

describe('CreateLinkedCollectionModal', () => {
  it('creates the collection once the contract checks out as an NFT on the picked network', async () => {
    checkMock.mockResolvedValue('ok')
    const { onSubmit } = renderModal()
    expect(screen.getByTestId('linked-collection-submit')).toBeDisabled()

    await fillForm()
    await waitFor(() => expect(screen.getByTestId('linked-collection-submit')).toBeEnabled(), { timeout: 3000 })
    expect(checkMock).toHaveBeenCalledWith('amoy', CONTRACT)

    await userEvent.click(screen.getByTestId('linked-collection-submit'))
    expect(onSubmit).toHaveBeenCalledWith('Bored Hats', { network: 'amoy', address: CONTRACT })
  })

  it('explains a contract that is not an ERC-721 or ERC-1155 and keeps the form closed', async () => {
    checkMock.mockResolvedValue('not_nft')
    renderModal()
    await fillForm()
    expect(await screen.findByTestId('linked-contract-error', {}, { timeout: 3000 })).toBeInTheDocument()
    expect(screen.getByTestId('linked-collection-submit')).toBeDisabled()
  })

  it('refuses anything that cannot become an address as it is typed', async () => {
    renderModal()
    await userEvent.type(screen.getByTestId('linked-contract-address-input'), '0xzz')
    expect(screen.getByTestId('linked-contract-address-input')).toHaveValue('0x')
  })

  it('tells a contract already linked elsewhere apart from any other failed save', () => {
    renderModal({
      error: new BuilderServerError('duplicate key', 409, 'linkedContract_linkedNetwork_thirdPartyId_unique')
    })
    expect(screen.getByTestId('linked-collection-error')).toHaveTextContent(/already linked/i)
  })
})
