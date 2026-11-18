import { describe, it, expect } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { CurationStatus, type Collection } from '~/lib/collections'
import { CollectionStatusPill } from './CollectionStatusPill'

const underReview: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: '0xabc',
  urn: 'urn:decentraland:amoy:collections-v2:0xcontract',
  contractAddress: '0xcontract',
  isPublished: true,
  isApproved: false,
  itemCount: 2,
  minters: [],
  managers: [],
  createdAt: 1000,
  updatedAt: 1000
}

const published: Collection = { ...underReview, isApproved: true }

function renderPill(props: Parameters<typeof CollectionStatusPill>[0]) {
  const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>
  return render(<CollectionStatusPill {...props} />, { wrapper })
}

describe('CollectionStatusPill', () => {
  it('shows the hint as an (i) tooltip inside the pill', async () => {
    renderPill({ collection: underReview, hint: 'We’ll notify you once the review is complete' })
    expect(screen.getByTestId('collection-status')).toHaveAttribute('data-status', 'under_review')

    await userEvent.hover(screen.getByTestId('collection-status-hint-trigger'))
    expect(await screen.findByRole('tooltip')).toHaveTextContent(/notify you/i)
  })

  it('renders only the status when there is no hint', () => {
    renderPill({ collection: published })
    expect(screen.getByTestId('collection-status')).toHaveAttribute('data-status', 'published')
    expect(screen.queryByTestId('collection-status-hint-trigger')).not.toBeInTheDocument()
  })

  it('reads Needs fixes from the latest review request', () => {
    renderPill({ collection: { ...underReview, curationStatus: CurationStatus.REJECTED } })
    expect(screen.getByTestId('collection-status')).toHaveAttribute('data-status', 'rejected')
    expect(screen.getByTestId('collection-status')).toHaveTextContent(/needs fixes/i)
  })
})
