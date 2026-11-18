import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { ReviewReplyBadge } from './ReviewReplyBadge'

const recent = vi.hoisted(() => ({ events: [] as { type: string; actor: string }[], asked: [] as boolean[] }))
vi.mock('~/hooks/useCollectionEvents', () => ({
  useRecentCollectionEvents: (_address: string, _id: string, enabled: boolean) => {
    recent.asked.push(enabled)
    return { data: enabled ? recent.events : undefined }
  }
}))

const underReview = {
  id: 'c1',
  owner: '0xme',
  managers: [],
  minters: [],
  isPublished: true,
  isApproved: false,
  createdAt: 1,
  reviewedAt: 1
} as unknown as Collection

function renderBadge(collection: Collection) {
  render(
    <TranslationProvider>
      <ReviewReplyBadge collection={collection} address="0xme" />
    </TranslationProvider>
  )
}

describe('ReviewReplyBadge', () => {
  it('marks a collection under review whose curator is waiting for an answer', () => {
    recent.events = [{ type: 'review.comment', actor: 'curator' }]
    renderBadge(underReview)
    expect(screen.getByTestId('review-reply-badge')).toHaveTextContent('Curator comment')
  })

  it('shows nothing once the creator answered', () => {
    recent.events = [
      { type: 'review.comment', actor: 'creator' },
      { type: 'review.comment', actor: 'curator' }
    ]
    renderBadge(underReview)
    expect(screen.queryByTestId('review-reply-badge')).toBeNull()
  })

  it("asks for no timeline of a collection the user only sends items of: the server won't show it", () => {
    recent.asked = []
    renderBadge({ ...underReview, owner: '0xowner', minters: ['0xme'] })
    expect(recent.asked).toEqual([false])
  })

  it('asks for no timeline of a draft', () => {
    recent.asked = []
    renderBadge({ ...underReview, isPublished: false })
    expect(recent.asked).toEqual([false])
    expect(screen.queryByTestId('review-reply-badge')).toBeNull()
  })
})
