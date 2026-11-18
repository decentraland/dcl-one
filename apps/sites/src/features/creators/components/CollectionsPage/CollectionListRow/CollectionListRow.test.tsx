import { describe, it, expect, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { CollectionListRow } from './CollectionListRow'

vi.mock('~/lib/builder', () => ({
  fetchCollectionItemPreviews: vi
    .fn()
    .mockResolvedValue([{ id: 'i1', name: 'Hat', thumbnailUrl: 'https://cdn.example/1.png' }]),
  deleteCollection: vi.fn().mockResolvedValue(undefined)
}))
vi.mock('~/lib/clipboard', () => ({ copyToClipboard: vi.fn().mockResolvedValue(true) }))

import { copyToClipboard } from '~/lib/clipboard'

vi.mock('~/store/wallet', () => ({
  useWallet: (selector: (state: { session: { address: string } }) => unknown) =>
    selector({ session: { address: '0xabc' } })
}))

const NOW = Date.now()

const collection: Collection = {
  id: 'a1b2',
  name: 'Pirate Hats',
  owner: '0xabc',
  urn: 'urn:decentraland:matic:collections-v2:0xcontract',
  isPublished: true,
  isApproved: true,
  itemCount: 250,
  minters: [],
  managers: [],
  createdAt: NOW - 60 * 24 * 3_600_000,
  updatedAt: NOW - 21 * 24 * 3_600_000
}

function renderRow(overrides: Partial<Collection> = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <TranslationProvider>
        <MemoryRouter initialEntries={['/collections']}>
          <Routes>
            <Route path="/collections" element={children} />
            <Route path="/collections/:collectionId" element={<div data-testid="detail-page" />} />
          </Routes>
        </MemoryRouter>
      </TranslationProvider>
    </QueryClientProvider>
  )
  return render(<CollectionListRow collection={{ ...collection, ...overrides }} />, { wrapper })
}

describe('CollectionListRow', () => {
  it('shows the collection name, item count, status and dates', () => {
    renderRow()
    expect(screen.getByText('Pirate Hats')).toBeInTheDocument()
    expect(screen.getByTestId('collection-row-items')).toHaveTextContent('250 Items')
    expect(screen.getByTestId('collection-status')).toHaveTextContent(/published/i)
    expect(screen.getByTestId('collection-row-updated')).toHaveTextContent('3 weeks ago')
    expect(screen.getByTestId('collection-row-created')).toHaveTextContent(/\d{4}/)
  })

  it('badges collections the signed-in address only collaborates on', () => {
    renderRow({ owner: '0xother', managers: ['0xabc'] })
    expect(screen.getByTestId('collection-role')).toHaveTextContent(/collaborator/i)
  })

  it("shows no role badge on the address's own collections", () => {
    renderRow()
    expect(screen.queryByTestId('collection-role')).not.toBeInTheDocument()
  })

  it('links the row to the collection detail', async () => {
    renderRow()
    const link = screen.getByRole('link', { name: 'Pirate Hats' })
    expect(link).toHaveAttribute('href', '/collections/a1b2')
    await userEvent.click(link)
    expect(screen.getByTestId('detail-page')).toBeInTheDocument()
  })

  it('offers copy URN and copy address for a published collection', async () => {
    renderRow()
    await userEvent.click(screen.getByRole('button', { name: 'Collection actions' }))
    expect(screen.queryByTestId('delete-collection')).not.toBeInTheDocument()
    expect(screen.queryByTestId('manage-minters')).not.toBeInTheDocument()
    expect(screen.queryByTestId('manage-collaborators')).not.toBeInTheDocument()
    expect(screen.getByTestId('copy-address')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('copy-urn'))
    expect(copyToClipboard).toHaveBeenCalledWith(collection.urn)
    expect(screen.queryByTestId('detail-page')).not.toBeInTheDocument()
  })

  it('offers only deletion for a draft', async () => {
    renderRow({ isPublished: false, isApproved: false })
    await userEvent.click(screen.getByRole('button', { name: 'Collection actions' }))
    expect(screen.getByTestId('delete-collection')).toBeInTheDocument()
    expect(screen.queryByTestId('copy-urn')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('delete-collection'))
    expect(screen.getByTestId('delete-collection-modal')).toBeInTheDocument()
  })
})
