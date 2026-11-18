import { describe, it, expect, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { CollectionCard } from './CollectionCard'

vi.mock('~/lib/builder', () => ({
  fetchCollectionItemPreviews: vi.fn().mockResolvedValue([
    { id: 'i1', name: 'Hat', thumbnailUrl: 'https://cdn.example/1.png' },
    { id: 'i2', name: 'Cap', thumbnailUrl: 'https://cdn.example/2.png' }
  ])
}))

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

function renderCard(overrides: Partial<Collection> = {}) {
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
  return render(<CollectionCard collection={{ ...collection, ...overrides }} />, { wrapper })
}

describe('CollectionCard', () => {
  it('shows the collection name, item count, status and last update', async () => {
    renderCard()
    expect(screen.getByText('Pirate Hats')).toBeInTheDocument()
    expect(screen.getByTestId('collection-card-items')).toHaveTextContent('250 Items')
    expect(screen.getByTestId('collection-status')).toHaveTextContent(/published/i)
    expect(screen.getByTestId('collection-card-updated')).toHaveTextContent('Updated 3 weeks ago')
    await waitFor(() => expect(screen.getAllByTestId('collection-mosaic-cell')).toHaveLength(2))
  })

  it('derives the status badge from the publish/approve flags', () => {
    renderCard({ isPublished: false, isApproved: false })
    expect(screen.getByTestId('collection-status')).toHaveAttribute('data-status', 'draft')
  })

  it('badges collections the signed-in address collaborates on or sends for, but not its own', () => {
    const { unmount } = renderCard({ owner: '0xother', managers: ['0xABC'] })
    expect(screen.getByTestId('collection-role')).toHaveTextContent(/collaborator/i)
    unmount()
    renderCard({ owner: '0xother', minters: ['0xabc'] })
    expect(screen.getByTestId('collection-role')).toHaveTextContent(/sender/i)
  })

  it("shows no role badge on the address's own collections", () => {
    renderCard({ managers: ['0xabc'] })
    expect(screen.queryByTestId('collection-role')).not.toBeInTheDocument()
  })

  it('opens the collection detail on click', async () => {
    renderCard()
    await userEvent.click(screen.getByTestId('collection-card'))
    expect(screen.getByTestId('detail-page')).toBeInTheDocument()
  })

  it('opens the collection detail with the keyboard', async () => {
    renderCard()
    screen.getByTestId('collection-card').focus()
    await userEvent.keyboard('{Enter}')
    expect(screen.getByTestId('detail-page')).toBeInTheDocument()
  })

  it('marks a linked collection as Linked', () => {
    renderCard({ urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats' })
    expect(screen.getByTestId('collection-status')).toHaveAttribute('data-status', 'linked')
  })
})
