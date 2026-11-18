import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { useNotifications } from '~/lib/notifications'
import { CollectionActionsMenu } from './CollectionActionsMenu'

vi.mock('~/lib/builder', () => ({ deleteCollection: vi.fn() }))
vi.mock('~/lib/clipboard', () => ({ copyToClipboard: vi.fn().mockResolvedValue(true) }))
vi.mock('~/lib/navigation', () => ({ openExternal: vi.fn() }))
vi.mock('~/hooks/useFeatureFlag', () => ({ useFeatureFlag: () => ({ enabled: true, isLoading: false }) }))

import { deleteCollection } from '~/lib/builder'
import { copyToClipboard } from '~/lib/clipboard'
import { openExternal } from '~/lib/navigation'

const OWNER = '0xabc'

const draft: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: OWNER,
  urn: 'urn:decentraland:amoy:collections-v2:0xcontract',
  contractAddress: '0xcontract',
  isPublished: false,
  isApproved: false,
  itemCount: 2,
  minters: [],
  managers: [],
  createdAt: 1000,
  updatedAt: 1000
}

const onDeleted = vi.fn()

function renderMenu(
  collection: Collection,
  address = OWNER,
  props: Partial<Parameters<typeof CollectionActionsMenu>[0]> = {}
) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <TranslationProvider>
        <MemoryRouter initialEntries={['/collections/c1']}>
          <Routes>
            <Route path="/collections/c1" element={children} />
            <Route path="/collections" element={<div data-testid="collections-page" />} />
          </Routes>
        </MemoryRouter>
      </TranslationProvider>
    </QueryClientProvider>
  )
  return render(<CollectionActionsMenu collection={collection} address={address} onDeleted={onDeleted} {...props} />, {
    wrapper
  })
}

async function openMenu() {
  await userEvent.click(screen.getByTestId('collection-actions'))
  return screen.getByTestId('collection-actions-menu')
}

function stubViewport(compact: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockReturnValue({ matches: compact, addEventListener: vi.fn(), removeEventListener: vi.fn() })
  )
}

afterEach(() => vi.unstubAllGlobals())

describe('CollectionActionsMenu', () => {
  beforeEach(() => {
    useNotifications.setState({ toasts: [] })
    ;(deleteCollection as Mock).mockReset()
    ;(copyToClipboard as Mock).mockClear()
    onDeleted.mockClear()
  })

  it('offers only deletion for a draft, and deletes after confirmation', async () => {
    ;(deleteCollection as Mock).mockResolvedValue(undefined)
    renderMenu(draft)
    await openMenu()
    expect(screen.getByTestId('delete-collection')).toBeInTheDocument()
    expect(screen.queryByTestId('copy-urn')).not.toBeInTheDocument()
    expect(screen.queryByTestId('manage-senders')).not.toBeInTheDocument()
    expect(screen.queryByTestId('transfer-ownership')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('delete-collection'))
    expect(screen.getByTestId('delete-collection-modal-description')).toHaveTextContent('Pirate Hats')
    await userEvent.click(screen.getByTestId('delete-collection-confirm'))

    await waitFor(() => expect(onDeleted).toHaveBeenCalledTimes(1))
    expect(deleteCollection).toHaveBeenCalledWith(OWNER, 'c1')
    expect(useNotifications.getState().toasts[0]?.message).toMatch(/deleted/i)
  })

  it('keeps the dialog open and reports the failure when deletion is rejected', async () => {
    ;(deleteCollection as Mock).mockRejectedValue(new Error('locked'))
    renderMenu(draft)
    await openMenu()
    await userEvent.click(screen.getByTestId('delete-collection'))
    await userEvent.click(screen.getByTestId('delete-collection-confirm'))
    await waitFor(() => expect(useNotifications.getState().toasts).toHaveLength(1))
    expect(screen.getByTestId('delete-collection-modal')).toBeInTheDocument()
    expect(onDeleted).not.toHaveBeenCalled()
  })

  it('renders nothing for a draft under the publish lock', () => {
    renderMenu({ ...draft, lock: Date.now() })
    expect(screen.queryByTestId('collection-actions')).not.toBeInTheDocument()
  })

  it('lets anyone with access copy the URN and address once the collection is on-chain', async () => {
    renderMenu({ ...draft, isPublished: true }, '0xmanager')
    await openMenu()
    expect(screen.queryByTestId('delete-collection')).not.toBeInTheDocument()
    expect(screen.queryByTestId('manage-collaborators')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('copy-urn'))
    expect(copyToClipboard).toHaveBeenCalledWith(draft.urn)
    await waitFor(() => expect(useNotifications.getState().toasts[0]?.message).toMatch(/urn/i))
    expect(screen.queryByTestId('collection-actions-menu')).not.toBeInTheDocument()

    await openMenu()
    await userEvent.click(screen.getByTestId('copy-address'))
    expect(copyToClipboard).toHaveBeenCalledWith('0xcontract')
  })

  it('links an approved collection to its Shop page', async () => {
    renderMenu({ ...draft, isPublished: true, isApproved: true, contractAddress: '0xc0ffee' }, '0xmanager')
    await openMenu()
    await userEvent.click(screen.getByTestId('view-in-shop'))
    expect(openExternal).toHaveBeenCalledWith('https://decentraland.zone/shop/collection/0xc0ffee')
  })

  it('offers no Shop link before the first approval', async () => {
    renderMenu({ ...draft, isPublished: true, contractAddress: '0xc0ffee' })
    await openMenu()
    expect(screen.queryByTestId('view-in-shop')).not.toBeInTheDocument()
  })

  it('opens the collaborators and senders lists for the owner of an on-chain collection', async () => {
    const onManageRoles = vi.fn()
    renderMenu({ ...draft, isPublished: true }, OWNER, { onManageRoles })
    await openMenu()
    await userEvent.click(screen.getByTestId('manage-collaborators'))
    expect(onManageRoles).toHaveBeenCalledWith('collaborators')
    await openMenu()
    await userEvent.click(screen.getByTestId('manage-senders'))
    expect(onManageRoles).toHaveBeenCalledWith('senders')
  })

  it('offers the owner of an on-chain collection to transfer it, as the last, destructive entry', async () => {
    const onTransferOwnership = vi.fn()
    renderMenu({ ...draft, isPublished: true }, OWNER, { onManageRoles: vi.fn(), onTransferOwnership })
    const menu = await openMenu()
    const items = Array.from(menu.querySelectorAll('[role="menuitem"]'))
    const transfer = screen.getByTestId('transfer-ownership')
    expect(items[items.length - 1]).toBe(transfer)
    expect(transfer).toHaveAttribute('data-tone', 'danger')
    await userEvent.click(transfer)
    expect(onTransferOwnership).toHaveBeenCalledTimes(1)
  })

  it('never offers to transfer a linked collection, which has no contract of its own', async () => {
    renderMenu({ ...draft, urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats', isPublished: true }, OWNER, {
      onManageRoles: vi.fn(),
      onTransferOwnership: vi.fn()
    })
    await openMenu()
    expect(screen.queryByTestId('transfer-ownership')).not.toBeInTheDocument()
  })

  it('never offers the transfer to a collaborator', async () => {
    renderMenu({ ...draft, isPublished: true, managers: ['0xdef'] }, '0xdef', { onTransferOwnership: vi.fn() })
    await openMenu()
    expect(screen.getByTestId('copy-urn')).toBeInTheDocument()
    expect(screen.queryByTestId('transfer-ownership')).not.toBeInTheDocument()
  })

  it('hides the role entries when asked, even for the owner', async () => {
    renderMenu({ ...draft, isPublished: true }, OWNER, {
      showRoles: false,
      onManageRoles: vi.fn(),
      onTransferOwnership: vi.fn()
    })
    await openMenu()
    expect(screen.getByTestId('copy-urn')).toBeInTheDocument()
    expect(screen.queryByTestId('manage-collaborators')).not.toBeInTheDocument()
    expect(screen.queryByTestId('manage-senders')).not.toBeInTheDocument()
    expect(screen.queryByTestId('transfer-ownership')).not.toBeInTheDocument()
  })

  it('closes with Escape', async () => {
    renderMenu(draft)
    await openMenu()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByTestId('collection-actions-menu')).not.toBeInTheDocument()
  })

  describe('on a small screen', () => {
    beforeEach(() => stubViewport(true))

    it('keeps copying and the role entries for the owner of an on-chain collection', async () => {
      renderMenu({ ...draft, isPublished: true }, OWNER, { onManageRoles: vi.fn(), showActivity: true })
      const menu = await openMenu()
      const ids = Array.from(menu.querySelectorAll('[role="menuitem"]')).map(el => el.getAttribute('data-testid'))
      expect(ids).toEqual(['copy-urn', 'copy-address', 'collection-activity', 'manage-collaborators', 'manage-senders'])
    })

    it('leaves the review timeline out where the page does not offer it', async () => {
      renderMenu({ ...draft, isPublished: true }, OWNER, { onManageRoles: vi.fn() })
      await openMenu()
      expect(screen.queryByTestId('collection-activity')).not.toBeInTheDocument()
    })

    it('renders nothing for a draft, since deleting is desktop-only', () => {
      renderMenu(draft)
      expect(screen.queryByTestId('collection-actions')).not.toBeInTheDocument()
    })

    it('never offers Preview in World, since the explorer is desktop-only', () => {
      renderMenu(draft, OWNER, { onPreviewInWorld: vi.fn() })
      expect(screen.queryByTestId('collection-actions')).not.toBeInTheDocument()
    })

    it('offers Preview items on a small screen, even for a draft', async () => {
      const onPreviewItems = vi.fn()
      renderMenu(draft, OWNER, { onPreviewItems })
      await openMenu()
      await userEvent.click(screen.getByTestId('preview-items-action'))
      expect(onPreviewItems).toHaveBeenCalled()
    })

    it('carries Send Items on a small screen, where the header button is hidden', async () => {
      const onSendItems = vi.fn()
      renderMenu({ ...draft, isPublished: true }, OWNER, { onSendItems })
      const menu = await openMenu()
      const ids = Array.from(menu.querySelectorAll('[role="menuitem"]')).map(el => el.getAttribute('data-testid'))
      expect(ids[0]).toBe('send-items-action')
      await userEvent.click(screen.getByTestId('send-items-action'))
      expect(onSendItems).toHaveBeenCalled()
    })

    it('carries Create discount on a small screen, where the header button is hidden', async () => {
      const onCreateDiscount = vi.fn()
      renderMenu({ ...draft, isPublished: true }, OWNER, { onCreateDiscount })
      await openMenu()
      await userEvent.click(screen.getByTestId('create-discount-action'))
      expect(onCreateDiscount).toHaveBeenCalled()
    })

    it('opens the forum post on a small screen, where the header button is hidden', async () => {
      renderMenu({ ...draft, isPublished: true }, OWNER, { forumLink: 'https://forum.decentraland.org/t/hats/77' })
      await openMenu()
      await userEvent.click(screen.getByTestId('forum-post-action'))
      expect(openExternal).toHaveBeenCalledWith('https://forum.decentraland.org/t/hats/77')
    })
  })

  it('leads with Preview in World on desktop and launches it', async () => {
    stubViewport(false)
    const onPreviewInWorld = vi.fn()
    renderMenu(draft, OWNER, { onPreviewInWorld })
    const menu = await openMenu()
    const ids = Array.from(menu.querySelectorAll('[role="menuitem"]')).map(el => el.getAttribute('data-testid'))
    expect(ids).toEqual(['preview-in-world-action', 'delete-collection'])
    await userEvent.click(screen.getByTestId('preview-in-world-action'))
    expect(onPreviewInWorld).toHaveBeenCalled()
  })

  it('never shows Preview items on desktop, where the Open Editor button covers it', async () => {
    stubViewport(false)
    renderMenu({ ...draft, isPublished: true }, OWNER, { onPreviewItems: vi.fn() })
    await openMenu()
    expect(screen.queryByTestId('preview-items-action')).not.toBeInTheDocument()
  })

  it('never shows Create discount on desktop, where the header button covers it', async () => {
    stubViewport(false)
    renderMenu({ ...draft, isPublished: true }, OWNER, { onCreateDiscount: vi.fn() })
    await openMenu()
    expect(screen.queryByTestId('create-discount-action')).not.toBeInTheDocument()
  })

  it('never shows Send Items on desktop, where the header button covers it', async () => {
    stubViewport(false)
    renderMenu({ ...draft, isPublished: true }, OWNER, { onSendItems: vi.fn() })
    await openMenu()
    expect(screen.queryByTestId('send-items-action')).not.toBeInTheDocument()
  })

  it('lets a manager copy or edit the URN of a draft linked collection and delete it', async () => {
    const onEditUrn = vi.fn()
    renderMenu({ ...draft, urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats' }, OWNER, { onEditUrn })
    await openMenu()
    expect(screen.getByTestId('copy-urn')).toBeInTheDocument()
    expect(screen.getByTestId('delete-collection')).toBeInTheDocument()
    expect(screen.queryByTestId('copy-address')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('edit-collection-urn'))
    expect(onEditUrn).toHaveBeenCalled()
  })

  it('lets a manager change the linked contract whether or not the collection is published', async () => {
    const onEditContract = vi.fn()
    renderMenu({ ...draft, urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats', isPublished: true }, OWNER, {
      onEditContract
    })
    await openMenu()
    await userEvent.click(screen.getByTestId('edit-linked-contract'))
    expect(onEditContract).toHaveBeenCalled()
  })

  it('locks the URN and the delete once the linked collection has published items', async () => {
    renderMenu({ ...draft, urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats', isPublished: true }, OWNER, {
      onEditUrn: vi.fn()
    })
    await openMenu()
    expect(screen.getByTestId('copy-urn')).toBeInTheDocument()
    expect(screen.queryByTestId('copy-address')).not.toBeInTheDocument()
    expect(screen.queryByTestId('edit-collection-urn')).not.toBeInTheDocument()
    expect(screen.queryByTestId('delete-collection')).not.toBeInTheDocument()
  })
})
